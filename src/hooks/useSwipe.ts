import { useRef, useCallback } from "react";

interface SwipeHandlers {
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
}

interface SwipeBind {
  onTouchStart: (e: React.TouchEvent) => void;
  onTouchEnd: (e: React.TouchEvent) => void;
}

const SWIPE_THRESHOLD = 80;

export function useSwipe(handlers: SwipeHandlers): SwipeBind {
  const startX = useRef(0);
  const startY = useRef(0);

  const onTouchStart = useCallback((e: React.TouchEvent) => {
    startX.current = e.touches[0].clientX;
    startY.current = e.touches[0].clientY;
  }, []);

  const onTouchEnd = useCallback(
    (e: React.TouchEvent) => {
      const endX = e.changedTouches[0].clientX;
      const endY = e.changedTouches[0].clientY;
      const dx = endX - startX.current;
      const dy = endY - startY.current;

      // 只处理水平滑动，忽略垂直滑动
      if (Math.abs(dx) < Math.abs(dy)) return;
      if (Math.abs(dx) < SWIPE_THRESHOLD) return;

      if (dx > 0 && handlers.onSwipeRight) {
        handlers.onSwipeRight();
      } else if (dx < 0 && handlers.onSwipeLeft) {
        handlers.onSwipeLeft();
      }
    },
    [handlers]
  );

  return { onTouchStart, onTouchEnd };
}