import { useEffect, useRef } from "react";
import { useLocation } from "react-router-dom";
import { useSpaceStore } from "../../stores/spaceStore";
import { voiceService } from "../../services/voice";
import { SpaceStatus } from "../../enum";

/**
 * 语音自动加入：进入某空间（/space/:id 及其子路由）且空间处于运行中时
 * 自动加入语音频道；离开空间路由或空间断开时自动退出。
 */
export function VoiceAutoJoin() {
  const location = useLocation();
  const { spaces } = useSpaceStore();
  const joinToken = useRef(0);

  const match = location.pathname.match(/^\/space\/([^/]+)/);
  const spaceId = match?.[1] ?? null;
  const space = spaces.find((s) => s.id === spaceId);
  const isRunning = space?.status === SpaceStatus.CED;

  useEffect(() => {
    const token = ++joinToken.current;
    const targetId = isRunning ? spaceId : null;

    if (!targetId) {
      void voiceService.leave();
      return;
    }

    (async () => {
      try {
        await voiceService.join(targetId);
      } catch (e) {
        console.error("[voice] auto join failed:", e);
      }
    })();

    return () => {
      // 仅最新一轮 effect 的清理负责退出，避免中间态频繁 join/leave
      if (token === joinToken.current) {
        void voiceService.leave();
      }
    };
  }, [spaceId, isRunning]);

  return null;
}
