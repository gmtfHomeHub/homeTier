// 移动端权限引导组件
// 覆盖：Android MediaProjection（屏幕投射）/ iOS ReplayKit（屏幕录制）/ 麦克风 / 相机
import React, { useState } from 'react';
import { observer } from 'mobx-react-lite';
import { Box, Text, Button, Flex, Progress } from '@radix-ui/themes';
import { useTranslation } from 'react-i18next';
import {
  AlertCircleIcon,
  CheckIcon,
  MonitorIcon,
  MicIcon,
  CameraIcon,
  ChevronLeftIcon,
  XIcon,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import './PermissionGuide.css';

export type PermissionType = 'mediaProjection' | 'replayKit' | 'microphone' | 'camera';

export interface PermissionGuideProps {
  type: PermissionType;
  onComplete: () => void;
  onDismiss: () => void;
  spaceId?: string;
}

interface GuideStep {
  id: number;
  title: string;
  description: string;
  /** 首步是否触发系统权限请求 */
  action?: 'button' | 'auto';
  actionLabel?: string;
}

const PERMISSION_TITLES: Record<PermissionType, string> = {
  mediaProjection: '屏幕投射权限',
  replayKit: '屏幕录制权限',
  microphone: '麦克风权限',
  camera: '相机权限',
};

const STEP_ICONS: Record<PermissionType, React.ReactNode> = {
  mediaProjection: <MonitorIcon size={48} />,
  replayKit: <MonitorIcon size={48} />,
  microphone: <MicIcon size={48} />,
  camera: <CameraIcon size={48} />,
};

const STEPS: Record<PermissionType, GuideStep[]> = {
  mediaProjection: [
    {
      id: 1,
      title: '需要屏幕投射权限',
      description: 'homeTier 需要使用 Android MediaProjection 捕获屏幕内容，实现屏幕共享。',
      action: 'button',
      actionLabel: '授予权限',
    },
    {
      id: 2,
      title: '确认系统弹窗',
      description: '点击授予后，系统会弹出「开始录制屏幕？」对话框，请点击「开始录制」。',
    },
    {
      id: 3,
      title: '开始共享',
      description: '权限授予后，屏幕共享将自动开始，您可以在工具栏随时停止。',
    },
  ],
  replayKit: [
    {
      id: 1,
      title: '需要屏幕录制权限',
      description: 'homeTier 使用 iOS ReplayKit 进行屏幕共享，需要授予屏幕录制权限。',
      action: 'button',
      actionLabel: '前往设置',
    },
    {
      id: 2,
      title: '开启屏幕录制',
      description: '在系统设置中找到「屏幕录制」，将 homeTier 设为允许。',
    },
    {
      id: 3,
      title: '启动广播',
      description: '返回应用，点击「开始广播」，选择 homeTier 扩展开始屏幕共享。',
    },
  ],
  microphone: [
    {
      id: 1,
      title: '需要麦克风权限',
      description: '语音通话需要访问麦克风采集您的声音。',
      action: 'button',
      actionLabel: '授予权限',
    },
    {
      id: 2,
      title: '确认系统弹窗',
      description: '系统会弹出权限请求，请点击「允许」。',
    },
    {
      id: 3,
      title: '加入语音',
      description: '权限授予后即可加入语音频道与队友通话。',
    },
  ],
  camera: [
    {
      id: 1,
      title: '需要相机权限',
      description: '视频通话或拍照功能需要访问相机。',
      action: 'button',
      actionLabel: '授予权限',
    },
    {
      id: 2,
      title: '确认系统弹窗',
      description: '系统会弹出权限请求，请点击「允许」。',
    },
    {
      id: 3,
      title: '开始使用',
      description: '权限授予后即可使用视频通话功能。',
    },
  ],
};

export const PermissionGuide = observer(function PermissionGuide({
  type,
  onComplete,
  onDismiss,
  spaceId,
}: PermissionGuideProps) {
  const { t } = useTranslation();
  const [currentStep, setCurrentStep] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const steps = STEPS[type];
  const current = steps[currentStep];
  const isLast = currentStep === steps.length - 1;
  const progress = ((currentStep + 1) / steps.length) * 100;

  /**
   * 触发系统权限请求。
   * - Rust 命令：移动端屏幕共享生命周期命令（未接入时静默跳过）
   * - Android 原生插件：真正的系统弹窗来源（MediaProjection 对话框 / 相机 / 麦克风运行时权限）
   */
  const triggerPermission = async () => {
    const rustCmd: Partial<Record<PermissionType, [string, Record<string, unknown>]>> = {
      mediaProjection: ['mobile_screen_request_permission', { spaceId: spaceId ?? '' }],
      replayKit: ['mobile_screen_open_settings', { spaceId: spaceId ?? '' }],
      camera: ['mobile_screen_request_camera_permission', { spaceId: spaceId ?? '' }],
    };
    // Android 原生插件命令（plugin:hometiervpnservice|xxx，snake_case）
    const pluginCmd: Partial<Record<PermissionType, [string, Record<string, unknown>]>> = {
      mediaProjection: ['plugin:hometiervpnservice|request_screen_capture', {}],
      microphone: ['plugin:hometiervpnservice|request_mic_permission', {}],
      camera: ['plugin:hometiervpnservice|request_camera_permission', {}],
    };
    const cmds = [rustCmd[type], pluginCmd[type]].filter(Boolean) as [
      string,
      Record<string, unknown>,
    ][];
    await Promise.all(cmds.map(([cmd, args]) => invoke(cmd, args).catch(() => undefined)));
  };

  const handlePrimary = async () => {
    setIsLoading(true);
    setError(null);
    try {
      if (currentStep === 0) {
        await triggerPermission();
        // 给系统权限弹窗留出展示时间
        await new Promise((resolve) => setTimeout(resolve, 600));
      }
      if (isLast) {
        onComplete();
      } else {
        setCurrentStep((step) => step + 1);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsLoading(false);
    }
  };

  const handleBack = () => {
    if (currentStep === 0) {
      onDismiss();
    } else {
      setCurrentStep((step) => step - 1);
    }
  };

  return (
    <Box
      className="permission-guide-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="guide-title"
    >
      <Box className="permission-guide-modal">
        {/* 头部 */}
        <Box className="guide-header">
          <Box className="guide-header-left">
            <Button variant="ghost" size="2" onClick={handleBack} aria-label={t('common.back')}>
              <ChevronLeftIcon size={20} />
            </Button>
            <Box>
              <Text weight="bold" size="3" id="guide-title" style={{ color: 'white' }}>
                {PERMISSION_TITLES[type]}
              </Text>
              <Text size="2" color="gray">
                {t('mobile.permission.step', { current: currentStep + 1, total: steps.length })}
              </Text>
            </Box>
          </Box>
          <Button variant="ghost" size="2" onClick={onDismiss} aria-label={t('common.close')}>
            <XIcon size={20} />
          </Button>
        </Box>

        {/* 进度条 + 步骤指示 */}
        <Box className="guide-progress">
          <Progress value={progress} max={100} size="2" color="blue" />
          <Flex justify="between" className="step-indicators">
            {steps.map((step, index) => (
              <Box
                key={step.id}
                className={`step-indicator ${index <= currentStep ? 'completed' : ''} ${
                  index === currentStep ? 'current' : ''
                }`}
              >
                <Box className="step-dot" />
                <Text size="1" color={index <= currentStep ? 'blue' : 'gray'}>
                  {step.id}
                </Text>
              </Box>
            ))}
          </Flex>
        </Box>

        {/* 内容区 */}
        <Box className="guide-content">
          <Box className="step-icon-wrapper">{STEP_ICONS[type]}</Box>

          <Text weight="bold" size="3" style={{ textAlign: 'center', marginBottom: 12, color: 'white' }}>
            {current.title}
          </Text>

          <Text
            size="2"
            color="gray"
            style={{ textAlign: 'center', lineHeight: 1.6, marginBottom: 24 }}
          >
            {current.description}
          </Text>

          {error && (
            <Box
              style={{
                marginBottom: 16,
                padding: '12px 16px',
                background: 'rgba(239, 68, 68, 0.15)',
                border: '1px solid rgba(239, 68, 68, 0.3)',
                borderRadius: 12,
                display: 'flex',
                alignItems: 'center',
                gap: 8,
              }}
            >
              <AlertCircleIcon size={16} color="#ef4444" />
              <Text size="2" color="red">
                {error}
              </Text>
            </Box>
          )}

          <Flex direction="column" gap="3" style={{ width: '100%' }}>
            <Button
              color="blue"
              size="3"
              onClick={handlePrimary}
              loading={isLoading}
              style={{ width: '100%', height: 56, fontSize: 16, fontWeight: 600 }}
            >
              {currentStep === 0 && current.actionLabel
                ? current.actionLabel
                : isLast
                  ? t('mobile.permission.done')
                  : t('mobile.permission.next')}
            </Button>
            <Button variant="ghost" size="2" onClick={handleBack} style={{ width: '100%' }}>
              {currentStep === 0 ? t('mobile.permission.cancel') : t('mobile.permission.back')}
            </Button>
          </Flex>
        </Box>

        {/* 底部说明 */}
        <Box className="guide-footer">
          <Text size="1" color="gray" style={{ textAlign: 'center' }}>
            {t('mobile.permission.footer')}
          </Text>
        </Box>
      </Box>
    </Box>
  );
});

export default PermissionGuide;
