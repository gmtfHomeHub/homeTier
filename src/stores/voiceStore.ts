import create from "zustand";

interface VoiceStore {
  joinedChannels: Record<string, boolean>;
  micMuted: boolean;
  speakerMuted: boolean;

  setJoined: (spaceId: string, joined: boolean) => void;
  toggleMic: () => void;
  toggleSpeaker: () => void;
}

type SetFn = (partial: VoiceStore | Partial<VoiceStore> | ((state: VoiceStore) => VoiceStore | Partial<VoiceStore>)) => void;

export const useVoiceStore = create<VoiceStore>((set: SetFn) => ({
  joinedChannels: {},
  micMuted: false,
  speakerMuted: false,

  setJoined: (spaceId: string, joined: boolean) =>
    set((state: VoiceStore) => ({
      joinedChannels: { ...state.joinedChannels, [spaceId]: joined },
    })),

  toggleMic: () => set((state: VoiceStore) => ({ micMuted: !state.micMuted })),

  toggleSpeaker: () => set((state: VoiceStore) => ({ speakerMuted: !state.speakerMuted })),
}));