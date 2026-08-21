// 移动端语音状态存储
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface MobileVoiceState {
  // 状态
  micMuted: boolean;
  speakerMuted: boolean;
  voiceStatus: 'disconnected' | 'connecting' | 'connected' | 'muted';
  
  // 动作
  setMicMuted: (muted: boolean) => void;
  setSpeakerMuted: (muted: boolean) => void;
  setVoiceStatus: (status: 'disconnected' | 'connecting' | 'connected' | 'muted') => void;
  
  toggleMic: () => Promise<boolean>;
  toggleSpeaker: () => Promise<boolean>;
  joinVoice: (spaceId: string) => Promise<void>;
  leaveVoice: (spaceId: string) => Promise<void>;
}

export const useMobileVoiceStore = create<MobileVoiceState>()(
  persist(
    (set, get) => ({
      micMuted: false,
      speakerMuted: false,
      voiceStatus: 'disconnected',
      
      setMicMuted: (muted) => set({ micMuted: muted }),
      setSpeakerMuted: (muted) => set({ speakerMuted: muted }),
      setVoiceStatus: (status) => set({ voiceStatus: status }),
      
      toggleMic: async () => {
        const newMuted = !get().micMuted;
        set({ micMuted: newMuted });
        return newMuted;
      },
      
      toggleSpeaker: async () => {
        const newMuted = !get().speakerMuted;
        set({ speakerMuted: newMuted });
        return newMuted;
      },
      
      joinVoice: async (spaceId: string) => {
        set({ voiceStatus: 'connecting' });
        try {
          // TODO: 调用后端命令
          // await invoke('mobile_voice_join', { spaceId });
          set({ voiceStatus: 'connected' });
        } catch (e) {
          set({ voiceStatus: 'disconnected' });
          throw e;
        }
      },
      
      leaveVoice: async (spaceId: string) => {
        // await invoke('mobile_voice_leave', { spaceId });
        set({ voiceStatus: 'disconnected' });
      },
    }),
    {
      name: 'mobile-voice-store',
      partialize: (state) => ({
        micMuted: state.micMuted,
        speakerMuted: state.speakerMuted,
      }),
    }
  )
);