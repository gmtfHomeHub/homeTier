import { useState, useEffect } from "react";
import { Button } from "@radix-ui/themes";
import { Monitor, MonitorOff, Eye } from "lucide-react";
import * as api from "../../utils/api";

export function ScreenShareView() {
  const [isSharing, setIsSharing] = useState(false);
  const [viewers, setViewers] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const sharing = await api.isScreenSharing();
        if (!cancelled) {
          setIsSharing(sharing);
          if (sharing) {
            const v = await api.getScreenShareViewers();
            if (!cancelled) setViewers(v);
          } else {
            setViewers([]);
          }
        }
      } catch {
        // ignore
      }
    };
    poll();
    const timer = setInterval(poll, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  const handleStart = async () => {
    try {
      await api.startScreenShare();
      setIsSharing(true);
    } catch (e) {
      console.error("Screen share start failed:", e);
    }
  };

  const handleStop = async () => {
    try {
      await api.stopScreenShare();
      setIsSharing(false);
      setViewers([]);
    } catch (e) {
      console.error("Screen share stop failed:", e);
    }
  };

  return (
    <div className="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-border)]">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Monitor size={20} className="text-[var(--color-primary)]" />
          <span className="font-medium text-sm">屏幕共享</span>
        </div>
        <Button
          onClick={isSharing ? handleStop : handleStart}
          variant={isSharing ? "solid" : "soft"}
          color={isSharing ? "red" : "blue"}
          size="2"
        >
          {isSharing ? (
            <span className="flex items-center gap-1">
              <MonitorOff size={14} />
              停止共享
            </span>
          ) : (
            <span className="flex items-center gap-1">
              <Monitor size={14} />
              开始共享
            </span>
          )}
        </Button>
      </div>

      {isSharing && (
        <div className="space-y-2">
          <div className="text-xs text-[var(--color-text-secondary)]">
            正在共享屏幕
          </div>
          {viewers.length > 0 && (
            <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
              <Eye size={12} />
              <span>{viewers.length} 人正在查看</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}