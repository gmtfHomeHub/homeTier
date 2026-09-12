// 移动端屏幕共享状态存储
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const useMobileScreenStore = create<
  { 
    isSharing: boolean;
    screenStatus: 'disconnected' | 'connecting' | 'connected' | 'paused';
    quality: 'low' | 'medium' | 'high' | 'ultra';
    setIsSharing: (sharing: boolean) => void;
    setScreenStatus: (status: 'disconnected' | 'connecting' | 'connected' | 'paused') => void;
    setQuality: (quality: 'low' | 'medium' | 'high' | 'ultra') => void;
    startSharing: (spaceId: string) => Promise<void>;
    stopSharing: (spaceId: string) => Promise<void>;
    setQualityLevel: (quality: 'low' | 'medium' | 'high' | 'ultra') => void;
  }
>()(
  persist(
    (set) => ({
      isSharing: false,
      screenStatus: 'disconnected',
      quality: 'medium',
      
      setIsSharing: (sharing) => set({ isSharing: sharing }),
      setScreenStatus: (status) => set({ screenStatus: status }),
      setQuality: (quality) => set({ quality }),
      
      startSharing: async (_spaceId: string) => {
        set({ screenStatus: 'connecting' });
        try {
          // await invoke('mobile_screen_start', { spaceId });
          set({ screenStatus: 'connected', isSharing: true });
        } catch (e) {
          set({ screenStatus: 'disconnected', isSharing: false });
          throw e;
        }
      },
      
      stopSharing: async (_spaceId: string) => {
        // await invoke('mobile_screen_stop', { spaceId });
        set({ screenStatus: 'disconnected', isSharing: false });
      },
      
      setQualityLevel: async (quality: 'low' | 'medium' | 'high' | 'ultra') => {
        set({ quality });
        // await invoke('mobile_screen_set_quality', { quality });
      },
    }),
    {
      name: 'mobile-screen-store',
      partialize: (state) => ({
        quality: state.quality,
      }),
    }
  )
);