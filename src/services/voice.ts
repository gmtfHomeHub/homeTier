import i18next from "i18next";
import { registerSignalHandler, sendSignal, preloadMembers, resolveMember, getSelfVirtualIp } from "./signal";
import { useVoiceStore } from "../stores/voiceStore";

/**
 * 实时语音服务（前端 WebRTC 网状连接）
 *
 * 媒体平面完全在前端：getUserMedia 采集、RTCPeerConnection 全网状互联、
 * Web Audio 播放/音量/VAD。后端仅通过 chat 消息通道（signal.ts，kind="voice"）
 * 转发信令：join/leave 广播，offer/answer/ice 定向。
 *
 * 回声消除/噪声抑制/自动增益由 getUserMedia 约束内置。
 */

const RTC_CONFIG: RTCConfiguration = { iceServers: [] };

const VAD_INTERVAL_MS = 150;
const VAD_SPEAKING_THRESHOLD = 0.045; // RMS 说话阈值
const VAD_SILENCE_MUTE_MS = 1200; // 连续静音多久后自动禁麦

interface RemotePeer {
  ip: string;
  pc: RTCPeerConnection;
  polite: boolean;
  makingOffer: boolean;
  ignoreOffer: boolean;
  isSettingRemoteAnswerPending: boolean;
  stream: MediaStream | null;
  pendingIce: RTCIceCandidateInit[];
  ctx: AudioContext;
  source: MediaStreamAudioSourceNode | null;
  analyser: AnalyserNode;
  gain: GainNode;
}

class VoiceService {
  private spaceId: string | null = null;
  private localStream: MediaStream | null = null;
  private localTrack: MediaStreamTrack | null = null;
  private audioCtx: AudioContext | null = null;
  private localAnalyser: AnalyserNode | null = null;
  private localBuffer: Uint8Array | null = null;
  private remoteBuffer: Uint8Array | null = null;
  private peers = new Map<string, RemotePeer>();
  private unregister: (() => void) | null = null;
  private vadTimer: ReturnType<typeof setInterval> | null = null;
  private volumeTimer: ReturnType<typeof setInterval> | null = null;
  private silentSince = 0;

  get joined(): boolean {
    return this.spaceId !== null;
  }

  async join(spaceId: string): Promise<void> {
    if (this.spaceId === spaceId) return;

    await this.leave();

    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error(i18next.t("voice.unsupported", "当前环境不支持麦克风采集（mediaDevices 不可用）"));
    }

    useVoiceStore.getState().setJoining(true);

    try {
      // 1. 采集本地麦克风（内置回声消除/噪声抑制/自动增益）
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      });
      this.localStream = stream;
      this.localTrack = stream.getAudioTracks()[0] ?? null;

      // 2. 建立 Web Audio 上下文，用于本地音量 / VAD
      this.audioCtx = new AudioContext();
      this.localAnalyser = this.audioCtx.createAnalyser();
      this.localAnalyser.fftSize = 1024;
      this.localBuffer = new Uint8Array(this.localAnalyser.fftSize);
      this.remoteBuffer = new Uint8Array(this.localAnalyser.fftSize);
      this.audioCtx.createMediaStreamSource(stream).connect(this.localAnalyser);

      this.spaceId = spaceId;
      useVoiceStore.getState().setJoined(true);
      useVoiceStore.getState().setJoining(false);
      useVoiceStore.getState().setMicMuted(false);
      useVoiceStore.getState().setSpeakerMuted(false);

      // 3. 预取成员列表（信令 from -> 昵称 解析）
      try {
        await preloadMembers(spaceId);
      } catch {
        // 成员列表获取失败不阻塞入会
      }

      // 4. 注册信令处理
      this.unregister = registerSignalHandler("voice", (sp, env) => {
        if (sp !== this.spaceId) return;
        void this.handleSignal(env);
      });

      // 5. 广播入会
      await sendSignal(spaceId, "voice", "join");

      // 6. 主动对已知成员建链（覆盖"对方已在线但未广播 join"的场景）
      await this.ensurePeersFromMembers(spaceId);

      // 7. 启动 VAD 与音量轮询
      this.silentSince = 0;
      this.vadTimer = setInterval(() => this.tickVad(), VAD_INTERVAL_MS);
      this.volumeTimer = setInterval(() => this.tickVolume(), 100);
    } catch (e) {
      // 入会失败：清理已获取的资源并复位状态
      this.spaceId = null;
      this.localTrack?.stop();
      this.localTrack = null;
      this.localStream = null;
      if (this.audioCtx) {
        void this.audioCtx.close();
        this.audioCtx = null;
      }
      this.localAnalyser = null;
      useVoiceStore.getState().setJoined(false);
      useVoiceStore.getState().setJoining(false);
      throw e;
    }
  }

  async leave(): Promise<void> {
    if (this.unregister) {
      this.unregister();
      this.unregister = null;
    }
    if (this.vadTimer) {
      clearInterval(this.vadTimer);
      this.vadTimer = null;
    }
    if (this.volumeTimer) {
      clearInterval(this.volumeTimer);
      this.volumeTimer = null;
    }

    if (this.spaceId) {
      try {
        await sendSignal(this.spaceId, "voice", "leave");
      } catch {
        // 广播失败不阻塞退出
      }
      useVoiceStore.getState().setJoined(false);
      useVoiceStore.getState().setJoining(false);
    }
    this.spaceId = null;

    for (const peer of this.peers.values()) {
      this.closePeer(peer);
    }
    this.peers.clear();
    useVoiceStore.getState().clearPeers();

    this.localTrack?.stop();
    this.localTrack = null;
    this.localStream = null;
    if (this.audioCtx) {
      void this.audioCtx.close();
      this.audioCtx = null;
    }
    this.localAnalyser = null;
  }

  async toggleMic(): Promise<boolean> {
    const store = useVoiceStore.getState();
    const next = !store.micMuted;
    store.setMicMuted(next);
    if (this.localTrack) this.localTrack.enabled = !next;
    if (this.spaceId) {
      try {
        await sendSignal(this.spaceId, "voice", "mic_state", { muted: next });
      } catch {
        // 广播失败不阻塞本地切换
      }
    }
    return next;
  }

  async toggleSpeaker(): Promise<boolean> {
    const store = useVoiceStore.getState();
    const next = !store.speakerMuted;
    store.setSpeakerMuted(next);
    for (const peer of this.peers.values()) {
      peer.gain.gain.value = next ? 0 : 1;
    }
    if (this.spaceId) {
      try {
        await sendSignal(this.spaceId, "voice", "speaker_state", { muted: next });
      } catch {
        // 广播失败不阻塞本地切换
      }
    }
    return next;
  }

  // === 信令处理 ===

  private async handleSignal(env: { type: string; from: string; data?: unknown }) {
    const { type, from, data } = env;
    if (!from) return;

    const selfIp = this.spaceId ? getSelfVirtualIp(this.spaceId) : undefined;
    if (selfIp && from === selfIp) return; // 自环防护

    switch (type) {
      case "join":
        await this.onPeerJoin(from);
        break;
      case "leave":
        this.onPeerLeave(from);
        break;
      case "offer":
        await this.onOffer(from, (data as { sdp?: string })?.sdp ?? "");
        break;
      case "answer":
        await this.onAnswer(from, (data as { sdp?: string })?.sdp ?? "");
        break;
      case "ice":
        await this.onIce(from, data as { candidate?: string } | undefined);
        break;
      case "mic_state": {
        const muted = Boolean((data as { muted?: boolean })?.muted);
        useVoiceStore.getState().setPeerMuted(from, muted);
        break;
      }
      case "speaker_state": {
        const muted = Boolean((data as { muted?: boolean })?.muted);
        useVoiceStore.getState().setPeerSpeakerMuted(from, muted);
        break;
      }
    }
  }

  private async onPeerJoin(remoteIp: string) {
    if (this.peers.has(remoteIp)) return;
    const peer = await this.createPeer(remoteIp);
    this.peers.set(remoteIp, peer);
    this.upsertPeerEntry(remoteIp);
    // 确定性 offerer：较小 IP 一侧发 offer，另一侧回复 answer，避免 glare
    const selfIp = this.spaceId ? getSelfVirtualIp(this.spaceId) : undefined;
    if ((selfIp ?? "") < remoteIp) {
      await this.sendOffer(peer);
    }
  }

  private async ensurePeersFromMembers(spaceId: string) {
    const { listMembers } = await import("../utils/api");
    let members: { virtual_ip?: string; is_online?: boolean }[];
    try {
      members = await listMembers(spaceId);
    } catch {
      return;
    }
    const selfIp = getSelfVirtualIp(spaceId);
    for (const m of members) {
      const ip = m.virtual_ip;
      if (!ip || ip === selfIp) continue;
      if (m.is_online === false) continue; // 仅对在线成员建链
      if (this.peers.has(ip)) continue;
      const peer = await this.createPeer(ip);
      this.peers.set(ip, peer);
      this.upsertPeerEntry(ip);
      if ((selfIp ?? "") < ip) {
        await this.sendOffer(peer);
      }
    }
  }

  private upsertPeerEntry(ip: string) {
    useVoiceStore.getState().upsertPeer(ip, {
      nickname: this.nicknameOf(ip),
      speaking: false,
      muted: false,
      speakerMuted: false,
      volume: 0,
    });
  }

  private onPeerLeave(remoteIp: string) {
    const peer = this.peers.get(remoteIp);
    if (peer) {
      this.closePeer(peer);
      this.peers.delete(remoteIp);
    }
    useVoiceStore.getState().removePeer(remoteIp);
  }

  private async onOffer(remoteIp: string, sdp: string) {
    let peer = this.peers.get(remoteIp);
    if (!peer) {
      peer = await this.createPeer(remoteIp);
      this.peers.set(remoteIp, peer);
      useVoiceStore.getState().upsertPeer(remoteIp, {
        nickname: this.nicknameOf(remoteIp),
        speaking: false,
        muted: false,
        speakerMuted: false,
        volume: 0,
      });
    }
    await peer.pc.setRemoteDescription({ type: "offer", sdp });
    await peer.pc.setLocalDescription();
    await this.flushPendingIce(peer);
    await sendSignal(this.spaceId!, "voice", "answer", { sdp: peer.pc.localDescription?.sdp ?? "" }, remoteIp);
  }

  private async onAnswer(remoteIp: string, sdp: string) {
    const peer = this.peers.get(remoteIp);
    if (!peer) return;
    await peer.pc.setRemoteDescription({ type: "answer", sdp });
    await this.flushPendingIce(peer);
  }

  private async onIce(remoteIp: string, data: { candidate?: string } | undefined) {
    const peer = this.peers.get(remoteIp);
    if (!peer) return;
    const candidate = data?.candidate;
    if (!candidate) return;
    try {
      const init = JSON.parse(candidate) as RTCIceCandidateInit;
      if (peer.pc.remoteDescription) {
        await peer.pc.addIceCandidate(init);
      } else {
        peer.pendingIce.push(init); // remoteDescription 未就绪前缓冲
      }
    } catch (e) {
      console.warn("[voice] addIceCandidate failed:", e);
    }
  }

  private async flushPendingIce(peer: RemotePeer) {
    if (!peer.pc.remoteDescription) return;
    const pending = peer.pendingIce.splice(0);
    for (const init of pending) {
      try {
        await peer.pc.addIceCandidate(init);
      } catch (e) {
        console.warn("[voice] buffered addIceCandidate failed:", e);
      }
    }
  }

  // === RTCPeerConnection 网状建链 ===

  private async createPeer(remoteIp: string): Promise<RemotePeer> {
    const selfIp = this.spaceId ? getSelfVirtualIp(this.spaceId) : undefined;
    const polite = (selfIp ?? "") < remoteIp;

    const pc = new RTCPeerConnection(RTC_CONFIG);
    if (this.localStream) {
      for (const track of this.localStream.getAudioTracks()) {
        pc.addTrack(track, this.localStream);
      }
    }

    const ctx = new AudioContext();
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 1024;
    const gain = ctx.createGain();
    gain.gain.value = useVoiceStore.getState().speakerMuted ? 0 : 1;
    analyser.connect(gain).connect(ctx.destination);

    const peer: RemotePeer = {
      ip: remoteIp,
      pc,
      polite,
      makingOffer: false,
      ignoreOffer: false,
      isSettingRemoteAnswerPending: false,
      stream: null,
      pendingIce: [],
      ctx,
      source: null,
      analyser,
      gain,
    };

    pc.onicecandidate = (e) => {
      if (e.candidate) {
        void sendSignal(this.spaceId!, "voice", "ice", { candidate: JSON.stringify(e.candidate.toJSON()) }, remoteIp);
      }
    };

    pc.ontrack = (e) => {
      const stream = e.streams[0] ?? new MediaStream([e.track]);
      peer.stream = stream;
      const src = ctx.createMediaStreamSource(stream);
      peer.source = src;
      src.connect(analyser);
      useVoiceStore.getState().setPeerStream(remoteIp, stream);
    };

    pc.onconnectionstatechange = () => {
      if (["failed", "disconnected", "closed"].includes(pc.connectionState)) {
        this.onPeerLeave(remoteIp);
      }
    };

    return peer;
  }

  private async sendOffer(peer: RemotePeer) {
    if (!this.spaceId) return;
    const offerOptions: RTCOfferOptions = { offerToReceiveAudio: true };
    await peer.pc.setLocalDescription(await peer.pc.createOffer(offerOptions));
    await sendSignal(this.spaceId, "voice", "offer", { sdp: peer.pc.localDescription?.sdp ?? "" }, peer.ip);
  }

  // === 音量 / VAD ===

  private tickVolume() {
    const store = useVoiceStore.getState();
    if (this.localAnalyser && this.localBuffer) {
      this.localAnalyser.getByteTimeDomainData(this.localBuffer);
      store.setLocalVolume(this.rms(this.localBuffer));
    }
    for (const [ip, peer] of this.peers) {
      const buf = this.remoteBuffer;
      if (!buf) continue;
      peer.analyser.getByteTimeDomainData(buf);
      const v = this.rms(buf);
      const speaking = v > VAD_SPEAKING_THRESHOLD;
      store.setPeerState(ip, { volume: v, speaking });
    }
  }

  private tickVad() {
    if (!this.localAnalyser || !this.localBuffer) return;
    this.localAnalyser.getByteTimeDomainData(this.localBuffer);
    const level = this.rms(this.localBuffer);
    const store = useVoiceStore.getState();
    const speaking = level > VAD_SPEAKING_THRESHOLD;
    store.setLocalSpeaking(speaking);
    if (speaking) {
      this.silentSince = 0;
      if (this.localTrack && !store.micMuted && !this.localTrack.enabled) {
        this.localTrack.enabled = true; // 检测到说话立即恢复
      }
    } else {
      this.silentSince += VAD_INTERVAL_MS;
      if (
        this.localTrack &&
        this.localTrack.enabled &&
        !store.micMuted &&
        this.silentSince >= VAD_SILENCE_MUTE_MS
      ) {
        this.localTrack.enabled = false; // 连续静音自动禁麦
      }
    }
  }

  private rms(data: Uint8Array): number {
    let sum = 0;
    for (let i = 0; i < data.length; i++) {
      const v = (data[i] - 128) / 128;
      sum += v * v;
    }
    return Math.sqrt(sum / data.length);
  }

  private nicknameOf(ip: string): string {
    if (!this.spaceId) return ip;
    const m = resolveMember(this.spaceId, ip);
    return m?.nickname ?? ip;
  }

  private closePeer(peer: RemotePeer) {
    try {
      peer.source?.disconnect();
    } catch {
      // ignore
    }
    void peer.ctx.close();
    peer.pc.close();
    peer.stream?.getTracks().forEach((t) => t.stop());
  }
}

export const voiceService = new VoiceService();
