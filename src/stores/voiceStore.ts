import { create } from "zustand";

export interface VoicePeer {
  ip: string;
  nickname: string;
  speaking: boolean;
  muted: boolean;
  speakerMuted: boolean;
  volume: number;
  stream: MediaStream | null;
}

interface VoiceState {
  joined: boolean;
  joining: boolean;
  micMuted: boolean;
  speakerMuted: boolean;
  localVolume: number;
  localSpeaking: boolean;
  peers: Record<string, VoicePeer>;

  setJoined: (joined: boolean) => void;
  setJoining: (joining: boolean) => void;
  setMicMuted: (muted: boolean) => void;
  setSpeakerMuted: (muted: boolean) => void;
  setLocalVolume: (v: number) => void;
  setLocalSpeaking: (v: boolean) => void;
  upsertPeer: (ip: string, peer: Partial<VoicePeer>) => void;
  setPeerMuted: (ip: string, muted: boolean) => void;
  setPeerSpeakerMuted: (ip: string, muted: boolean) => void;
  setPeerStream: (ip: string, stream: MediaStream) => void;
  setPeerState: (ip: string, state: { volume?: number; speaking?: boolean }) => void;
  removePeer: (ip: string) => void;
  clearPeers: () => void;
  reset: () => void;
}

export const useVoiceStore = create<VoiceState>((set) => ({
  joined: false,
  joining: false,
  micMuted: false,
  speakerMuted: false,
  localVolume: 0,
  localSpeaking: false,
  peers: {},

  setJoined: (joined) => set({ joined }),
  setJoining: (joining) => set({ joining }),
  setMicMuted: (micMuted) => set({ micMuted }),
  setSpeakerMuted: (speakerMuted) => set({ speakerMuted }),
  setLocalVolume: (localVolume) => set({ localVolume }),
  setLocalSpeaking: (localSpeaking) => set({ localSpeaking }),

  upsertPeer: (ip, peer) =>
    set((state) => {
      const existing = state.peers[ip] ?? {
        ip,
        nickname: ip,
        speaking: false,
        muted: false,
        speakerMuted: false,
        volume: 0,
        stream: null,
      };
      return { peers: { ...state.peers, [ip]: { ...existing, ...peer } } };
    }),

  setPeerMuted: (ip, muted) =>
    set((state) => {
      const existing = state.peers[ip];
      if (!existing) return {};
      return { peers: { ...state.peers, [ip]: { ...existing, muted } } };
    }),

  setPeerSpeakerMuted: (ip, speakerMuted) =>
    set((state) => {
      const existing = state.peers[ip];
      if (!existing) return {};
      return { peers: { ...state.peers, [ip]: { ...existing, speakerMuted } } };
    }),

  setPeerStream: (ip, stream) =>
    set((state) => {
      const existing = state.peers[ip];
      if (!existing) return {};
      return { peers: { ...state.peers, [ip]: { ...existing, stream } } };
    }),

  setPeerState: (ip, state) =>
    set((prev) => {
      const existing = prev.peers[ip];
      if (!existing) return {};
      return { peers: { ...prev.peers, [ip]: { ...existing, ...state } } };
    }),

  removePeer: (ip) =>
    set((state) => {
      const peers = { ...state.peers };
      delete peers[ip];
      return { peers };
    }),

  clearPeers: () => set({ peers: {} }),

  reset: () =>
    set({
      joined: false,
      joining: false,
      micMuted: false,
      speakerMuted: false,
      localVolume: 0,
      localSpeaking: false,
      peers: {},
    }),
}));
