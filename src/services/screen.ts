import { registerSignalHandler, sendSignal, preloadMembers, resolveMember, getSelfVirtualIp } from "./signal";
import { useScreenStore, type ScreenQuality } from "../stores/screenStore";
import { toastError } from "../utils/toast";

/**
 * 屏幕共享服务（前端 WebRTC）
 *
 * 共享者（sharer）通过 getDisplayMedia 采集屏幕/窗口，作为唯一 offerer 与每个
 * 查看者（viewer）建立独立 RTCPeerConnection；查看者仅 answer + ontrack 显示。
 * 信令走 chat 通道（signal.ts，kind="screen"）：
 *   start / stop / quality  广播
 *   request_view            查看者广播请求接入
 *   offer / answer / ice    定向
 *
 * 画质切换通过 applyConstraints（分辨率/帧率）+ sender.setParameters（码率）实现。
 */

const RTC_CONFIG: RTCConfiguration = { iceServers: [] };

export const SCREEN_QUALITY_PRESETS: Record<
  ScreenQuality,
  { labelKey: string; width: number; height: number; frameRate: number; maxBitrate: number }
> = {
  smooth: { labelKey: "screen.qualitySmooth", width: 1280, height: 720, frameRate: 15, maxBitrate: 800_000 },
  standard: { labelKey: "screen.qualityStandard", width: 1920, height: 1080, frameRate: 30, maxBitrate: 2_500_000 },
  hd: { labelKey: "screen.qualityHd", width: 1920, height: 1080, frameRate: 60, maxBitrate: 5_000_000 },
};

interface ScreenPeer {
  ip: string;
  pc: RTCPeerConnection;
  pendingIce: RTCIceCandidateInit[];
  stream: MediaStream | null;
}

type ScreenMode = "idle" | "sharing" | "watching";

class ScreenService {
  private spaceId: string | null = null;
  private mode: ScreenMode = "idle";
  private localStream: MediaStream | null = null;
  private localTrack: MediaStreamTrack | null = null;
  private allowedIps: string[] = [];
  private peers = new Map<string, ScreenPeer>();
  private unregister: (() => void) | null = null;

  get isSharing(): boolean {
    return this.mode === "sharing";
  }

  get isWatching(): boolean {
    return this.mode === "watching";
  }

  /** 共享者：开始共享（allowedIps 空数组 = 允许所有人查看） */
  async startShare(spaceId: string, allowedIps: string[], quality: ScreenQuality): Promise<void> {
    if (this.mode === "sharing" && this.spaceId === spaceId) return;

    await this.stopShare();
    await this.stopWatching();

    if (!navigator.mediaDevices?.getDisplayMedia) {
      throw new Error("当前环境不支持屏幕采集（getDisplayMedia 不可用）");
    }

    const preset = SCREEN_QUALITY_PRESETS[quality];
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({
        video: { width: { ideal: preset.width }, height: { ideal: preset.height }, frameRate: { ideal: preset.frameRate } },
        audio: false,
      });
    } catch (e) {
      toastError("屏幕采集被取消或失败");
      throw e;
    }

    const track = stream.getVideoTracks()[0];
    if (!track) {
      stream.getTracks().forEach((t) => t.stop());
      toastError("未获取到屏幕视频轨道");
      throw new Error("未获取到屏幕视频轨道");
    }
    // 用户通过系统 UI 停止共享时自动清理
    track.addEventListener("ended", () => {
      void this.stopShare();
    });

    this.spaceId = spaceId;
    this.mode = "sharing";
    this.localStream = stream;
    this.localTrack = track;
    this.allowedIps = allowedIps;

    const store = useScreenStore.getState();
    store.setIsSharing(true);
    store.setSourceName(track.label || "screen");
    store.setQuality(quality);
    store.setViewerCount(0);

    try {
      await preloadMembers(spaceId);
    } catch {
      // ignore
    }

    this.unregister = registerSignalHandler("screen", (sp, env) => {
      if (sp !== this.spaceId) return;
      void this.handleSignal(env);
    });

    await sendSignal(spaceId, "screen", "start", { sourceName: track.label || "screen" });
  }

  /** 共享者：停止共享 */
  async stopShare(): Promise<void> {
    if (this.mode !== "sharing") {
      if (this.spaceId) {
        this.spaceId = null;
        this.unregister?.();
        this.unregister = null;
      }
      return;
    }
    const spaceId = this.spaceId;
    if (spaceId) {
      try {
        await sendSignal(spaceId, "screen", "stop");
      } catch {
        // ignore
      }
    }
    for (const peer of this.peers.values()) this.closePeer(peer);
    this.peers.clear();

    this.localTrack?.stop();
    this.localStream?.getTracks().forEach((t) => t.stop());
    this.localTrack = null;
    this.localStream = null;
    this.allowedIps = [];

    const store = useScreenStore.getState();
    store.setIsSharing(false);
    store.setSourceName("");
    store.setViewerCount(0);

    this.unregister?.();
    this.unregister = null;
    this.spaceId = null;
    this.mode = "idle";
  }

  /** 查看者：切换画质 */
  async setQuality(quality: ScreenQuality): Promise<void> {
    if (this.mode !== "sharing" || !this.spaceId) return;
    const preset = SCREEN_QUALITY_PRESETS[quality];
    useScreenStore.getState().setQuality(quality);

    // 1. 本地采集约束
    if (this.localTrack) {
      try {
        await this.localTrack.applyConstraints({
          width: { ideal: preset.width },
          height: { ideal: preset.height },
          frameRate: { ideal: preset.frameRate },
        });
      } catch (e) {
        console.warn("[screen] applyConstraints failed:", e);
      }
    }
    // 2. 每个 viewer 连接上的编码码率
    for (const peer of this.peers.values()) {
      for (const sender of peer.pc.getSenders()) {
        if (!sender.track) continue;
        try {
          const params = sender.getParameters();
          params.encodings = params.encodings.map((e) => ({ ...e, maxBitrate: preset.maxBitrate }));
          await sender.setParameters(params);
        } catch (e) {
          console.warn("[screen] setParameters failed:", e);
        }
      }
    }
    // 3. 广播画质变更
    try {
      await sendSignal(this.spaceId, "screen", "quality", { level: quality });
    } catch {
      // ignore
    }
  }

  /** 查看者：开始观看（进入 /space/:id/screen 页面时调用） */
  async startWatching(spaceId: string): Promise<void> {
    if (this.mode === "watching" && this.spaceId === spaceId) return;

    await this.stopShare();
    await this.stopWatching();

    this.spaceId = spaceId;
    this.mode = "watching";

    const store = useScreenStore.getState();
    store.setWatching(true);
    store.setRemoteStream(null);
    store.setShareEnded(false);

    try {
      await preloadMembers(spaceId);
    } catch {
      // ignore
    }

    this.unregister = registerSignalHandler("screen", (sp, env) => {
      if (sp !== this.spaceId) return;
      void this.handleSignal(env);
    });

    // 请求所有在播共享者接入
    await sendSignal(spaceId, "screen", "request_view");
  }

  /** 查看者：停止观看 */
  async stopWatching(): Promise<void> {
    if (this.mode !== "watching") {
      if (this.spaceId) {
        this.spaceId = null;
        this.unregister?.();
        this.unregister = null;
      }
      return;
    }
    for (const peer of this.peers.values()) this.closePeer(peer);
    this.peers.clear();

    const store = useScreenStore.getState();
    store.setWatching(false);
    store.setRemoteStream(null);
    store.setSharer(null, null);
    store.setShareEnded(false);

    this.unregister?.();
    this.unregister = null;
    this.spaceId = null;
    this.mode = "idle";
  }

  // === 信令处理 ===

  private async handleSignal(env: { type: string; from: string; data?: unknown }) {
    const { type, from, data } = env;
    if (!from) return;

    if (this.mode === "sharing") {
      await this.handleSharerSignal(type, from, data);
    } else if (this.mode === "watching") {
      await this.handleViewerSignal(type, from, data);
    }
  }

  private async handleSharerSignal(type: string, from: string, data?: unknown) {
    switch (type) {
      case "request_view": {
        // 权限过滤：空 = 所有人可看
        if (this.allowedIps.length > 0 && !this.allowedIps.includes(from)) return;
        if (this.peers.has(from)) return;
        await this.onViewerRequest(from);
        break;
      }
      case "answer":
        await this.onAnswer(from, (data as { sdp?: string })?.sdp ?? "");
        break;
      case "ice":
        await this.onIce(from, data as { candidate?: string } | undefined);
        break;
    }
  }

  private async handleViewerSignal(type: string, from: string, data?: unknown) {
    switch (type) {
      case "start": {
        const store = useScreenStore.getState();
        store.setShareEnded(false);
        store.setSharer(from, this.nicknameOf(from));
        break;
      }
      case "quality": {
        const level = (data as { level?: ScreenQuality })?.level;
        if (level) useScreenStore.getState().setRemoteQuality(level);
        break;
      }
      case "offer":
        await this.onOffer(from, (data as { sdp?: string })?.sdp ?? "");
        break;
      case "ice":
        await this.onIce(from, data as { candidate?: string } | undefined);
        break;
      case "stop": {
        const store = useScreenStore.getState();
        store.setShareEnded(true);
        store.setRemoteStream(null);
        store.setSharer(null, null);
        const peer = this.peers.get(from);
        if (peer) {
          this.closePeer(peer);
          this.peers.delete(from);
        }
        break;
      }
    }
  }

  // === 共享者侧：viewer 接入 ===

  private async onViewerRequest(viewerIp: string) {
    const peer = await this.createPeer(viewerIp);
    this.peers.set(viewerIp, peer);
    useScreenStore.getState().setViewerCount(this.peers.size);
    await this.sendOffer(peer);
  }

  // === 查看者侧：offer 处理 ===

  private async onOffer(sharerIp: string, sdp: string) {
    let peer = this.peers.get(sharerIp);
    if (!peer) {
      peer = await this.createPeer(sharerIp);
      this.peers.set(sharerIp, peer);
    }
    await peer.pc.setRemoteDescription({ type: "offer", sdp });
    await peer.pc.setLocalDescription(await peer.pc.createAnswer());
    await this.flushPendingIce(peer);
    await sendSignal(this.spaceId!, "screen", "answer", { sdp: peer.pc.localDescription?.sdp ?? "" }, sharerIp);
    const store = useScreenStore.getState();
    store.setSharer(sharerIp, this.nicknameOf(sharerIp));
    store.setShareEnded(false);
  }

  private async onAnswer(sharerIp: string, sdp: string) {
    const peer = this.peers.get(sharerIp);
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
        peer.pendingIce.push(init);
      }
    } catch (e) {
      console.warn("[screen] addIceCandidate failed:", e);
    }
  }

  private async flushPendingIce(peer: ScreenPeer) {
    if (!peer.pc.remoteDescription) return;
    const pending = peer.pendingIce.splice(0);
    for (const init of pending) {
      try {
        await peer.pc.addIceCandidate(init);
      } catch (e) {
        console.warn("[screen] buffered addIceCandidate failed:", e);
      }
    }
  }

  // === RTCPeerConnection ===

  private async createPeer(remoteIp: string): Promise<ScreenPeer> {
    const pc = new RTCPeerConnection(RTC_CONFIG);
    const peer: ScreenPeer = { ip: remoteIp, pc, pendingIce: [], stream: null };

    if (this.mode === "sharing" && this.localStream) {
      for (const track of this.localStream.getTracks()) {
        pc.addTrack(track, this.localStream);
      }
    }

    pc.onicecandidate = (e) => {
      if (e.candidate) {
        void sendSignal(this.spaceId!, "screen", "ice", { candidate: JSON.stringify(e.candidate.toJSON()) }, remoteIp);
      }
    };

    pc.ontrack = (e) => {
      const stream = e.streams[0] ?? new MediaStream([e.track]);
      peer.stream = stream;
      if (this.mode === "watching") {
        useScreenStore.getState().setRemoteStream(stream);
        useScreenStore.getState().setShareEnded(false);
      }
    };

    pc.onconnectionstatechange = () => {
      if (["failed", "disconnected", "closed"].includes(pc.connectionState)) {
        this.peers.delete(remoteIp);
        if (this.mode === "sharing") {
          useScreenStore.getState().setViewerCount(this.peers.size);
        } else if (this.mode === "watching") {
          useScreenStore.getState().setRemoteStream(null);
        }
        this.closePeer(peer);
      }
    };

    return peer;
  }

  private async sendOffer(peer: ScreenPeer) {
    if (!this.spaceId) return;
    await peer.pc.setLocalDescription(await peer.pc.createOffer());
    await sendSignal(this.spaceId, "screen", "offer", { sdp: peer.pc.localDescription?.sdp ?? "" }, peer.ip);
  }

  private nicknameOf(ip: string): string {
    if (!this.spaceId) return ip;
    const m = resolveMember(this.spaceId, ip);
    return m?.nickname ?? ip;
  }

  private closePeer(peer: ScreenPeer) {
    peer.pc.close();
    peer.stream?.getTracks().forEach((t) => t.stop());
  }
}

export const screenService = new ScreenService();
