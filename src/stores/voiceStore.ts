import create from "zustand";

interface VoiceStore {
  joinedChannels: Record<string, boolean>;
  micMuted: boolean;
  speakerMuted: boolean;
  speakingMembers: Record<string, boolean>;

  setJoined: (spaceId: string, joined: boolean) => void;
  toggleMic: () => void;
  toggleSpeaker: () => void;
  setSpeaking: (memberId: string, speaking: boolean) => void;
}

type SetFn = (partial: VoiceStore | Partial<VoiceStore> | ((state: VoiceStore) => VoiceStore | Partial<VoiceStore>)) => void;

export const useVoiceStore = create<VoiceStore>((set: SetFn) => ({
  joinedChannels: {},
  micMuted: false,
  speakerMuted: false,
  speakingMembers: {},

  setJoined: (spaceId: string, joined: boolean) =>
    set((state: VoiceStore) => ({
      joinedChannels: { ...state.joinedChannels, [spaceId]: joined },
    })),

  toggleMic: () => set((state: VoiceStore) => ({ micMuted: !state.micMuted })),

  toggleSpeaker: () => set((state: VoiceStore) => ({ speakerMuted: !state.speakerMuted })),

  setSpeaking: (memberId: string, speaking: boolean) =>
    set((state: VoiceStore) => ({
      speakingMembers: { ...state.speakingMembers, [memberId]: speaking },
    })),
}));