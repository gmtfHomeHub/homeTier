import { useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Button, Badge } from "@radix-ui/themes";
import { Monitor, MonitorOff, X } from "lucide-react";
import { useScreenStore, type ScreenQuality } from "../../stores/screenStore";
import { screenService, SCREEN_QUALITY_PRESETS } from "../../services/screen";

export function ScreenViewer() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();

  const watching = useScreenStore((s) => s.watching);
  const sharerName = useScreenStore((s) => s.sharerName);
  const remoteStream = useScreenStore((s) => s.remoteStream);
  const remoteQuality = useScreenStore((s) => s.remoteQuality);
  const shareEnded = useScreenStore((s) => s.shareEnded);

  useEffect(() => {
    if (!id) return;
    void screenService.startWatching(id);
    return () => {
      void screenService.stopWatching();
    };
  }, [id]);

  return (
    <div className="relative flex-1 bg-black overflow-hidden">
      {/* 视频 */}
      {remoteStream ? (
        <video
          ref={(el) => {
            if (el && remoteStream && el.srcObject !== remoteStream) {
              el.srcObject = remoteStream;
            }
          }}
          autoPlay
          playsInline
          className="w-full h-full object-contain"
        />
      ) : (
        <div className="w-full h-full flex flex-col items-center justify-center text-[var(--color-text-secondary)]">
          <MonitorOff size={48} className="mb-3 opacity-50" />
          <p className="text-sm">
            {shareEnded ? t("screen.shareEnded") : watching ? t("screen.waitingShare") : t("screen.notWatching")}
          </p>
        </div>
      )}

      {/* 右上角信息条 */}
      <div className="absolute top-3 right-3 flex items-center gap-2">
        {sharerName && (
          <div className="flex items-center gap-2 bg-black/60 backdrop-blur rounded-full px-3 py-1.5 text-white text-xs">
            <Monitor size={14} />
            <span className="truncate max-w-[160px]">{sharerName}</span>
          </div>
        )}
        {remoteStream && (
          <Badge size="1" color="blue" variant="soft">
            {t(SCREEN_QUALITY_PRESETS[(remoteQuality || "standard") as ScreenQuality].labelKey)}
          </Badge>
        )}
        <Button
          onClick={() => id && navigate(`/space/${id}`)}
          variant="solid"
          color="gray"
          size="1"
          className="bg-black/60"
        >
          <X size={14} />
          {t("screen.exitView")}
        </Button>
      </div>
    </div>
  );
}
