import { useParams } from "react-router-dom";
import { useVoiceStore } from "../../stores/voiceStore";
import { Mic, MicOff, Volume2, VolumeX, PhoneOff } from "lucide-react";
import { useNavigate } from "react-router-dom";
import * as api from "../../utils/api";
import { Button } from "@radix-ui/themes";

export function VoicePanel() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { micMuted, speakerMuted, joinedChannels, setJoined, toggleMic, toggleSpeaker } =
    useVoiceStore();

  const isJoined = id ? joinedChannels[id] : false;

  const handleJoin = async () => {
    if (!id) return;
    try {
      await api.joinVoiceChannel(id);
      setJoined(id, true);
    } catch (e) {
      console.error("Join voice failed:", e);
    }
  };

  const handleLeave = async () => {
    if (!id) return;
    try {
      await api.leaveVoiceChannel(id);
      setJoined(id, false);
    } catch (e) {
      console.error("Leave voice failed:", e);
    }
  };

  const handleToggleMic = async () => {
    if (!id) return;
    try {
      await api.toggleMic(id);
      toggleMic();
    } catch (e) {
      console.error("Toggle mic failed:", e);
    }
  };

  const handleToggleSpeaker = async () => {
    if (!id) return;
    try {
      await api.toggleSpeaker(id);
      toggleSpeaker();
    } catch (e) {
      console.error("Toggle speaker failed:", e);
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6">
      <div className="bg-[var(--color-surface)] rounded-2xl p-8 w-80 border border-[var(--color-border)] text-center">
        <div className="text-4xl mb-4">🎤</div>
        <h2 className="text-lg font-semibold mb-2">语音频道</h2>
        <p className="text-sm text-[var(--color-text-secondary)] mb-6">
          {isJoined ? "已加入语音频道" : "点击加入语音频道"}
        </p>

        {isJoined ? (
          <div className="space-y-4">
            {/* 成员状态占位 */}
            <div className="flex justify-center gap-4 text-[var(--color-text-secondary)] text-xs">
              <span className="flex items-center gap-1">
                <div className="w-2 h-2 rounded-full bg-[var(--color-success)]" />
                自己
              </span>
            </div>

            {/* 控制按钮 */}
            <div className="flex justify-center gap-4">
              <Button
                onClick={handleToggleMic}
                variant={micMuted ? "solid" : "soft"}
                color={micMuted ? "red" : "blue"}
                size="3"
                className="p-4 rounded-full"
              >
                {micMuted ? <MicOff size={24} /> : <Mic size={24} />}
              </Button>
              <Button
                onClick={handleToggleSpeaker}
                variant={speakerMuted ? "solid" : "soft"}
                color={speakerMuted ? "red" : "blue"}
                size="3"
                className="p-4 rounded-full"
              >
                {speakerMuted ? <VolumeX size={24} /> : <Volume2 size={24} />}
              </Button>
              <Button
                onClick={handleLeave}
                variant="solid"
                color="red"
                size="3"
                className="p-4 rounded-full"
              >
                <PhoneOff size={24} />
              </Button>
            </div>
          </div>
        ) : (
          <Button
            onClick={handleJoin}
            variant="solid"
            color="blue"
            size="2"
          >
            加入语音
          </Button>
        )}

        <Button
          onClick={() => navigate(`/space/${id}`)}
          variant="ghost"
          size="1"
        >
          返回聊天
        </Button>
      </div>
    </div>
  );
}