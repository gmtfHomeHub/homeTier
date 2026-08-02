import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Mic,
  MicOff,
  Volume2,
  VolumeX,
  PhoneOff,
  ArrowLeft,
  Loader2,
} from "lucide-react";
import { Button } from "@radix-ui/themes";
import { useVoiceStore } from "../../stores/voiceStore";
import { voiceService } from "../../services/voice";
import { toastError } from "../../utils/toast";

export function VoicePanel() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();

  const { joined, joining, micMuted, speakerMuted, localVolume, localSpeaking, peers } =
    useVoiceStore();

  const members = Object.values(peers);
  const hasMembers = members.length > 0;

  const handleJoin = async () => {
    if (!id) return;
    try {
      await voiceService.join(id);
    } catch (e) {
      toastError(String(e));
    }
  };

  const handleLeave = async () => {
    if (!id) return;
    try {
      await voiceService.leave();
    } catch (e) {
      toastError(String(e));
    }
  };

  const handleToggleMic = async () => {
    try {
      await voiceService.toggleMic();
    } catch (e) {
      toastError(String(e));
    }
  };

  const handleToggleSpeaker = async () => {
    try {
      await voiceService.toggleSpeaker();
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6">
      <div className="bg-[var(--color-surface)] rounded-2xl p-6 w-[420px] max-w-full border border-[var(--color-border)]">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t("voice.title")}</h2>
          <Button onClick={() => navigate(`/space/${id}`)} variant="ghost" size="1">
            <ArrowLeft size={16} />
          </Button>
        </div>

        <p className="text-sm text-[var(--color-text-secondary)] mb-4">
          {joined ? t("voice.joined") : t("voice.notJoined")}
        </p>

        {/* 成员列表（含自己） */}
        <div className="space-y-2 mb-5">
          {joined && (
            <div className="flex items-center gap-3 px-3 py-2 rounded-xl bg-[var(--color-bg)] border border-[var(--color-border)]">
              <div
                className={`w-2 h-2 rounded-full shrink-0 ${
                  localSpeaking && !micMuted ? "bg-green-500 animate-pulse" : "bg-[var(--color-text-secondary)]"
                }`}
              />
              <span className="flex-1 text-sm truncate">
                {t("voice.you")}
                {micMuted && (
                  <span className="ml-2 text-xs text-red-400">
                    <MicOff size={12} className="inline" /> {t("voice.micMuted")}
                  </span>
                )}
              </span>
              <VolumeBar volume={localVolume} />
            </div>
          )}

          {members.map((p) => (
            <div
              key={p.ip}
              className="flex items-center gap-3 px-3 py-2 rounded-xl bg-[var(--color-bg)] border border-[var(--color-border)]"
            >
              <div
                className={`w-2 h-2 rounded-full shrink-0 ${
                  p.speaking && !p.muted ? "bg-green-500 animate-pulse" : "bg-[var(--color-text-secondary)]"
                }`}
              />
              <span className="flex-1 text-sm truncate">
                {p.nickname}
                {p.muted && (
                  <span className="ml-2 text-xs text-red-400">
                    <MicOff size={12} className="inline" /> {t("voice.micMuted")}
                  </span>
                )}
              </span>
              <VolumeBar volume={p.volume} />
            </div>
          ))}

          {joined && !hasMembers && (
            <p className="text-xs text-[var(--color-text-secondary)] text-center py-2">
              {t("voice.noMembers")}
            </p>
          )}
        </div>

        {/* 控制按钮 */}
        {joined ? (
          <div className="flex justify-center gap-4">
            <Button
              onClick={handleToggleMic}
              variant={micMuted ? "solid" : "soft"}
              color={micMuted ? "red" : "blue"}
              size="3"
              className="p-4 rounded-full"
              title={micMuted ? t("voice.unmuteMic") : t("voice.muteMic")}
            >
              {micMuted ? <MicOff size={22} /> : <Mic size={22} />}
            </Button>
            <Button
              onClick={handleToggleSpeaker}
              variant={speakerMuted ? "solid" : "soft"}
              color={speakerMuted ? "red" : "blue"}
              size="3"
              className="p-4 rounded-full"
              title={speakerMuted ? t("voice.unmuteSpeaker") : t("voice.muteSpeaker")}
            >
              {speakerMuted ? <VolumeX size={22} /> : <Volume2 size={22} />}
            </Button>
            <Button
              onClick={handleLeave}
              variant="solid"
              color="red"
              size="3"
              className="p-4 rounded-full"
              title={t("voice.leave")}
            >
              <PhoneOff size={22} />
            </Button>
          </div>
        ) : (
          <div className="flex justify-center">
            <Button onClick={handleJoin} variant="solid" color="blue" size="2" disabled={joining}>
              {joining ? <Loader2 size={16} className="animate-spin" /> : null}
              {t("voice.join")}
            </Button>
          </div>
        )}

        <p className="text-center text-xs text-[var(--color-text-secondary)] mt-4">
          {t("voice.vadHint")}
        </p>
      </div>
    </div>
  );
}

function VolumeBar({ volume }: { volume: number }) {
  const width = Math.min(100, Math.round(volume * 100 * 4));
  return (
    <div className="w-16 h-1.5 bg-[var(--color-border)] rounded-full overflow-hidden shrink-0">
      <div
        className={`h-full rounded-full ${width > 0 ? "bg-blue-500" : ""}`}
        style={{ width: `${width}%`, transition: "width 100ms linear" }}
      />
    </div>
  );
}
