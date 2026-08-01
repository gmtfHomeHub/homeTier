import { create } from "zustand";

export type ScreenQuality = "smooth" | "standard" | "hd";

interface ScreenState {
  isSharing: boolean;
  sourceName: string;
  quality: ScreenQuality;
  viewerCount: number;
  error: string | null;

  watching: boolean;
  sharerIp: string | null;
  sharerName: string | null;
  remoteStream: MediaStream | null;
  remoteQuality: ScreenQuality;
  shareEnded: boolean;

  setIsSharing: (v: boolean) => void;
  setSourceName: (name: string) => void;
  setQuality: (q: ScreenQuality) => void;
  setViewerCount: (n: number) => void;
  setError: (e: string | null) => void;
  setWatching: (v: boolean) => void;
  setSharer: (ip: string | null, name: string | null) => void;
  setRemoteStream: (s: MediaStream | null) => void;
  setRemoteQuality: (q: ScreenQuality) => void;
  setShareEnded: (v: boolean) => void;
  reset: () => void;
}

export const useScreenStore = create<ScreenState>((set) => ({
  isSharing: false,
  sourceName: "",
  quality: "standard",
  viewerCount: 0,
  error: null,

  watching: false,
  sharerIp: null,
  sharerName: null,
  remoteStream: null,
  remoteQuality: "standard",
  shareEnded: false,

  setIsSharing: (isSharing) => set({ isSharing }),
  setSourceName: (sourceName) => set({ sourceName }),
  setQuality: (quality) => set({ quality }),
  setViewerCount: (viewerCount) => set({ viewerCount }),
  setError: (error) => set({ error }),
  setWatching: (watching) => set({ watching }),
  setSharer: (sharerIp, sharerName) => set({ sharerIp, sharerName }),
  setRemoteStream: (remoteStream) => set({ remoteStream }),
  setRemoteQuality: (remoteQuality) => set({ remoteQuality }),
  setShareEnded: (shareEnded) => set({ shareEnded }),
  reset: () =>
    set({
      isSharing: false,
      sourceName: "",
      quality: "standard",
      viewerCount: 0,
      error: null,
      watching: false,
      sharerIp: null,
      sharerName: null,
      remoteStream: null,
      remoteQuality: "standard",
      shareEnded: false,
    }),
}));
