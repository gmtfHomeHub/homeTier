// 移动端语音/屏幕共享控制工具栏
import React, { useState } from 'react';
import { observer } from 'mobx-react-lite';
import { Box, Button, Text, Flex } from '@radix-ui/themes';
import { useTranslation } from 'react-i18next';
import {
  MicIcon,
  MicOffIcon,
  Volume2,
  VolumeX,
  MonitorIcon,
  MonitorOffIcon,
  XIcon,
  SettingsIcon,
} from 'lucide-react';
import { useMobileVoiceStore } from '../../stores/mobileVoiceStore';
import { useMobileScreenStore } from '../../stores/mobileScreenStore';
import { toastInfo, toastError } from '../../utils/toast';
import './MobileVoiceToolbar.css';

interface MobileVoiceToolbarProps {
  spaceId: string;
  onClose?: () => void;
}

export const MobileVoiceToolbar = observer(({ spaceId, onClose }: MobileVoiceToolbarProps) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  // 语音状态
  const { micMuted, speakerMuted, voiceStatus, toggleMic, toggleSpeaker } = useMobileVoiceStore();

  // 屏幕共享状态
  const { isSharing, screenStatus, startSharing, stopSharing, setQuality, quality } =
    useMobileScreenStore();

  const isConnected = voiceStatus === 'connected' || screenStatus === 'connected';
  const isConnecting = voiceStatus === 'connecting' || screenStatus === 'connecting';

  // 格式化时长
  const formatDuration = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  const handleToggleMic = async () => {
    try {
      const muted = await toggleMic();
      toastInfo(muted ? t('mobile.voice.muted') : t('mobile.voice.active'));
    } catch (e) {
      toastError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleToggleSpeaker = async () => {
    try {
      const muted = await toggleSpeaker();
      toastInfo(muted ? t('mobile.voice.speaker_off') : t('mobile.voice.speaker_on'));
    } catch (e) {
      toastError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleToggleShare = async () => {
    try {
      if (isSharing) {
        await stopSharing(spaceId);
      } else {
        await startSharing(spaceId);
      }
    } catch (e) {
      toastError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleClose = () => {
    onClose?.();
    setExpanded(false);
  };

  return (
    <Box
      className={`mobile-voice-toolbar ${expanded ? 'expanded' : ''} ${
        isConnected ? 'connected' : ''
      }`}
    >
      {/* 收起状态 - 浮动按钮 */}
      <Box className="toolbar-float" onClick={() => setExpanded(true)}>
        <Box
          className={`float-button ${isConnected ? 'active' : ''} ${
            isConnecting ? 'connecting' : ''
          }`}
        >
          <MonitorIcon size={24} />
          {isConnecting && <Box className="connecting-pulse" />}
        </Box>
        {isConnected && (
          <Box className="float-duration">
            <Text size="1" weight="medium" color="gray">
              {formatDuration(0)}
            </Text>
          </Box>
        )}
        <button
          type="button"
          className="icon-button"
          onClick={() => setExpanded(true)}
          aria-label={t('mobile.voice.toolbar')}
        >
          <SettingsIcon size={20} />
        </button>
      </Box>

      {/* 展开状态 - 底部工具栏 */}
      <Box className="toolbar-expanded">
        <Box className="toolbar-header">
          <Text weight="bold" size="2" color="gray">
            {t('mobile.voice.toolbar')}
          </Text>
          <button
            type="button"
            className="icon-button"
            onClick={handleClose}
            aria-label={t('common.close')}
          >
            <XIcon size={20} />
          </button>
        </Box>

        {/* 连接状态指示 */}
        <Box className="status-indicator">
          <Box
            className={`status-dot ${isConnected ? 'connected' : 'disconnected'} ${
              isConnecting ? 'connecting' : ''
            }`}
          />
          <Text size="2" color="gray" weight="medium">
            {isConnecting
              ? t('mobile.voice.connecting')
              : isConnected
                ? t('mobile.voice.connected')
                : t('mobile.voice.disconnected')}
          </Text>
        </Box>

        {/* 语音控制区域 */}
        <Box className="control-section">
          <Text size="2" weight="bold" color="gray" as="div" style={{ marginBottom: 12 }}>
            {t('mobile.voice.controls')}
          </Text>

          <Flex gap="3" align="center" justify="center">
            <Button
              variant={micMuted ? 'soft' : 'solid'}
              color={micMuted ? 'gray' : 'blue'}
              size="3"
              onClick={handleToggleMic}
              style={{ flex: 1, minWidth: 0, height: 56 }}
            >
              <Box style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}>
                {micMuted ? <MicOffIcon size={28} /> : <MicIcon size={28} />}
                <Text size="2" weight="medium">
                  {micMuted ? t('mobile.voice.muted') : t('mobile.voice.active')}
                </Text>
              </Box>
            </Button>

            <Button
              variant={speakerMuted ? 'soft' : 'solid'}
              color={speakerMuted ? 'gray' : 'green'}
              size="3"
              onClick={handleToggleSpeaker}
              style={{ flex: 1, minWidth: 0, height: 56 }}
            >
              <Box style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}>
                {speakerMuted ? <VolumeX size={28} /> : <Volume2 size={28} />}
                <Text size="2" weight="medium">
                  {speakerMuted ? t('mobile.voice.speaker_off') : t('mobile.voice.speaker_on')}
                </Text>
              </Box>
            </Button>
          </Flex>
        </Box>

        {/* 屏幕共享控制区域 */}
        <Box className="control-section">
          <Text size="2" weight="bold" color="gray" as="div" style={{ marginBottom: 12 }}>
            {t('mobile.screen.controls')}
          </Text>

          <Flex justify="center">
            <Button
              variant={isSharing ? 'solid' : 'outline'}
              color={isSharing ? 'red' : 'purple'}
              size="3"
              onClick={handleToggleShare}
              loading={screenStatus === 'connecting'}
              style={{ width: '100%', height: 56 }}
            >
              <Box style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}>
                {isSharing ? <MonitorOffIcon size={28} /> : <MonitorIcon size={28} />}
                <Text size="2" weight="medium">
                  {isSharing ? t('mobile.screen.sharing') : t('mobile.screen.start_sharing')}
                </Text>
              </Box>
            </Button>
          </Flex>

          {/* 画质选择 */}
          {isSharing && (
            <Flex gap="2" wrap="wrap" justify="center" style={{ marginTop: 8 }}>
              {(['low', 'medium', 'high', 'ultra'] as const).map((q) => (
                <Button
                  key={q}
                  variant={quality === q ? 'solid' : 'outline'}
                  color={quality === q ? 'purple' : 'gray'}
                  size="2"
                  onClick={() => setQuality(q)}
                  style={{ minWidth: 70 }}
                >
                  <Text size="2" weight="medium" as="span">
                    {t(`mobile.screen.quality.${q}`)}
                  </Text>
                </Button>
              ))}
            </Flex>
          )}
        </Box>

        {/* 连接信息 */}
        {isConnected && (
          <Box className="connection-info">
            <Flex gap="4" align="center" justify="center" style={{ padding: 8 }}>
              <Box className="info-item">
                <Text size="1" color="gray">
                  {t('mobile.voice.latency')}
                </Text>
                <Text size="2" weight="bold" color="gray">
                  45ms
                </Text>
              </Box>
              <Box className="divider" />
              <Box className="info-item">
                <Text size="1" color="gray">
                  {t('mobile.voice.quality')}
                </Text>
                <Text size="2" weight="bold" color="gray">
                  HD
                </Text>
              </Box>
              <Box className="divider" />
              <Box className="info-item">
                <Text size="1" color="gray">
                  {t('mobile.voice.duration')}
                </Text>
                <Text size="2" weight="bold" color="gray" id="toolbar-duration">
                  {formatDuration(0)}
                </Text>
              </Box>
            </Flex>
          </Box>
        )}
      </Box>
    </Box>
  );
});

export default MobileVoiceToolbar;
