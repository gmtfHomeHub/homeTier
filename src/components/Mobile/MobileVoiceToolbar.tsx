// 移动端语音控制工具栏组件
import React, { useState, useEffect } from 'react';
import { observer } from 'mobx-react-lite';
import { 
  Box, Button, Icon, Text, Stack, Badge, Tooltip, Flex 
} from '@radix-ui/themes';
import { useTranslation } from 'react-i18next';
import { 
  MicIcon, MicOffIcon, VolumeHighIcon, VolumeOffIcon, 
  MonitorIcon, MonitorOffIcon, XIcon, SettingsIcon
} from 'lucide-react';
import { useMobileVoiceStore } from '../../stores/mobileVoiceStore';
import { useMobileScreenStore } from '../../stores/mobileScreenStore';
import './MobileVoiceToolbar.css';

interface MobileVoiceToolbarProps {
  spaceId: string;
  onClose?: () => void;
}

export const MobileVoiceToolbar = observer(({ spaceId, onClose }: MobileVoiceToolbarProps) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  
  // 语音状态
  const {
    micMuted,
    speakerMuted,
    voiceStatus,
    toggleMic,
    toggleSpeaker,
    joinVoice,
    leaveVoice,
  } = useMobileVoiceStore();
  
  // 屏幕共享状态
  const {
    isSharing,
    screenStatus,
    startSharing,
    stopSharing,
    setQuality,
    quality,
  } = useMobileScreenStore();

  const isConnected = voiceStatus === 'connected' || screenStatus === 'connected';
  const isConnecting = voiceStatus === 'connecting' || screenStatus === 'connecting';

  // 格式化时长
  const formatDuration = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  const handleClose = () => {
    onClose?.();
    setExpanded(false);
  };

  return (
    <Box className={`mobile-voice-toolbar ${expanded ? 'expanded' : ''} ${isConnected ? 'connected' : ''}`}>
      {/* 收起状态 - 浮动按钮 */}
      <Box className="toolbar-float" onClick={() => setExpanded(true)}>
        <Box className={`float-button ${isConnected ? 'active' : ''} ${isConnecting ? 'connecting' : ''}`}>
          <MonitorIcon size={24} />
          {isConnecting && <Box className="connecting-pulse" />}
        </Box>
        {isConnected && (
          <Box className="float-duration">
            <Text size={1} weight="medium" color="white">
              00:00
            </Text>
          </Box>
        )}
        <Tooltip content={t('mobile.voice.toolbar')}>
          <IconButton onClick={() => setExpanded(true)}>
            <SettingsIcon size={20} />
          </IconButton>
        </Tooltip>
      </Box>

      {/* 展开状态 - 底部工具栏 */}
      <Box className="toolbar-expanded">
        <Box className="toolbar-header">
          <Text weight="bold" size={2} color="white">
            {t('mobile.voice.toolbar')}
          </Text>
          <IconButton onClick={handleClose} aria-label={t('common.close')}>
            <XIcon size={20} />
          </IconButton>
        </Box>

        {/* 连接状态指示 */}
        <Box className="status-indicator">
          <Box className={`status-dot ${isConnected ? 'connected' : 'disconnected'} ${isConnecting ? 'connecting' : ''}`} />
          <Text size={2} color="gray" weight="medium">
            {isConnecting ? t('mobile.voice.connecting') : 
              isConnected ? t('mobile.voice.connected') : t('mobile.voice.disconnected')}
          </Text>
        </Box>

        {/* 语音控制区域 */}
        <Box className="control-section">
          <Text size={2} weight="bold" color="white" as="div" style={{marginBottom: 12}}>
            {t('mobile.voice.controls')}
          </Text>
          
          <Flex gap={3} direction="column">
            <Flex gap={3} align="center" style={{flex: 1}}>
              <Tooltip content={micMuted ? t('mobile.voice.unmute') : t('mobile.voice.mute')}>
                <Button 
                  variant={micMuted ? 'soft' : 'solid'}
                  color={micMuted ? 'gray' : 'blue'}
                  size="3"
                  onClick={toggleMic}
                  style={{minWidth: 80, height: 56}}
                >
                  <Box style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4}}>
                    <micMuted ? <MicOffIcon size={28} /> : <MicIcon size={28} />}
                    <Text size={2} weight="medium">
                      {micMuted ? t('mobile.voice.muted') : t('mobile.voice.active')}
                    </Text>
                  </Box>
                </Button>
              </Tooltip>
              
              <Tooltip content={speakerMuted ? t('mobile.voice.unmute_speaker') : t('mobile.voice.mute_speaker')}>
                <Button 
                  variant={speakerMuted ? 'soft' : 'solid'}
                  color={speakerMuted ? 'gray' : 'green'}
                  size="3"
                  onClick={toggleSpeaker}
                  style={{minWidth: 80, height: 56}}
                >
                  <Box style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4}}>
                    <speakerMuted ? <VolumeOffIcon size={28} /> : <VolumeHighIcon size={28} />}
                    <Text size={2} weight="medium">
                      {speakerMuted ? t('mobile.voice.speaker_off') : t('mobile.voice.speaker_on')}
                    </Text>
                  </Box>
                </Button>
              </Tooltip>
            </Flex>
          </Flex>
        </Box>

        {/* 屏幕共享控制区域 */}
        <Box className="control-section">
          <Text size={2} weight="bold" color="white" as="div" style={{marginBottom: 12}}>
            {t('mobile.screen.controls')}
          </Text>
          
          <Flex gap={3} direction="column">
            <Flex gap={3} align="center" style={{flex: 1}}>
              <Tooltip content={isSharing ? t('mobile.screen.stop') : t('mobile.screen.start')}>
                <Button 
                  variant={isSharing ? 'solid' : 'outline'}
                  color={isSharing ? 'red' : 'purple'}
                  size="3"
                  onClick={isSharing ? stopSharing : startSharing}
                  isLoading={screenStatus === 'connecting'}
                  style={{minWidth: 120, height: 56}}
                >
                  <Box style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4}}>
                    <isSharing ? <MonitorOffIcon size={28} /> : <MonitorIcon size={28} />}
                    <Text size={2} weight="medium">
                      {isSharing ? t('mobile.screen.sharing') : t('mobile.screen.start_sharing')}
                    </Text>
                  </Box>
                </Button>
              </Tooltip>
            </Flex>
            
            {/* 画质选择 */}
            {isSharing && (
              <Flex gap={2} wrap justify="center" style={{marginTop: 8}}>
                {['low', 'medium', 'high', 'ultra'].map((q) => (
                  <Tooltip key={q} content={t(`mobile.screen.quality.${q}`)}>
                    <Button
                      variant={quality === q ? 'solid' : 'outline'}
                      color={quality === q ? 'purple' : 'gray'}
                      size="2"
                      onClick={() => setQuality(q as any)}
                      style={{minWidth: 70}}
                    >
                      <Text size={2} weight="medium" as="span">
                        {t(`mobile.screen.quality.${q}`)}
                      </Text>
                    </Button>
                  </Tooltip>
                ))}
              </Flex>
            )}
          </Flex>
        </Box>

        {/* 连接信息 */}
        {isConnected && (
          <Box className="connection-info">
            <Flex gap={4} align="center" justify="center" style={{padding: 8}}>
              <Box className="info-item">
                <Text size={1} color="gray">{t('mobile.voice.latency')}</Text>
                <Text size={2} weight="bold" color="white">45ms</Text>
              </Box>
              <Box className="divider" />
              <Box className="info-item">
                <Text size={1} color="gray">{t('mobile.voice.quality')}</Text>
                <Text size={2} weight="bold" color="white">HD</Text>
              </Box>
              <Box className="divider" />
              <Box className="info-item">
                <Text size={1} color="gray">{t('mobile.voice.duration')}</Text>
                <Text size={2} weight="bold" color="white" id="toolbar-duration">00:00</Text>
              </Box>
            </Flex>
          </Box>
        )}
      </Box>
    </Box>
  );
});

// 图标按钮组件
const IconButton = ({ onClick, children, ariaLabel, ...props }: any) => (
  <button 
    onClick={onClick}
    aria-label={ariaLabel}
    className="icon-button"
    {...props}
  >
    {children}
  </button>
);

export default MobileVoiceToolbar;