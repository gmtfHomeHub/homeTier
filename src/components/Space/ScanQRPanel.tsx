import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import jsQR from "jsqr";

interface ScanQRPanelProps {
  onResult: (text: string) => void;
  onCancel: () => void;
}

const SCAN_TIMEOUT_S = 30;
// 解码缩放上限：像素数降到 1/9，jsQR 在低端机也能 20-40ms/帧
const DECODE_MAX_DIM = 640;

/**
 * 基于 WebView getUserMedia + jsQR 的二维码扫描面板。
 *
 * 与原生 tauri-plugin-barcode-scanner 的区别：
 * - 全部 UI 在 DOM 内，错误/帧数/倒计时实时可见（原生 PreviewView 会遮挡 toast）
 * - 无原生视图，组件卸载即 stop tracks，不存在 cancel() 死代码 / PreviewView 泄漏
 * - CAMERA 权限：保留 barcode-scanner 插件 → manifest 合并 CAMERA；
 *   getUserMedia 触发 RustWebChromeClient.onPermissionRequest → permissionLauncher
 *   弹系统授权框（要求 manifest 已声明 CAMERA，html5-qrcode 时代正是缺这一步）
 */
export function ScanQRPanel({ onResult, onCancel }: ScanQRPanelProps) {
  const { t } = useTranslation();
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef = useRef<number | null>(null);
  const frameCountRef = useRef(0);
  // 解码循环用 ref 持有最新回调，避免其依赖变化导致 effect 重跑
  const onResultRef = useRef(onResult);
  const onCancelRef = useRef(onCancel);
  useEffect(() => {
    onResultRef.current = onResult;
    onCancelRef.current = onCancel;
  });

  const [error, setError] = useState<string | null>(null);
  const [display, setDisplay] = useState({ frames: 0, seconds: SCAN_TIMEOUT_S });

  // 拦截 Android 硬件返回键 → 取消扫描而非退出 Activity
  useEffect(() => {
    window.history.pushState({ qrScanning: true }, "");
    const onPopState = () => onCancelRef.current();
    window.addEventListener("popstate", onPopState);
    return () => {
      window.removeEventListener("popstate", onPopState);
      if (window.history.state?.qrScanning) window.history.back();
    };
  }, []);

  // 相机 + 解码主循环：仅依赖 t（无 onResult/onCancel，靠 ref）
  useEffect(() => {
    let stream: MediaStream | null = null;
    let stopped = false;

    const start = async () => {
      if (!navigator.mediaDevices?.getUserMedia) {
        setError(t("space.cameraNotSupported"));
        return;
      }
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment", width: { ideal: 1280 }, height: { ideal: 720 } },
          audio: false,
        });
      } catch (e) {
        setError(`${t("space.cameraUnavailable")}：${shortErr(e)}`);
        return;
      }
      if (stopped) {
        stream.getTracks().forEach((tr) => tr.stop());
        return;
      }
      const video = videoRef.current;
      if (!video) return;
      video.srcObject = stream;
      try {
        await video.play();
      } catch {
        // 静音 + playsInline 下 autoplay 通常无碍，忽略
      }

      const tick = () => {
        if (stopped) return;
        const v = videoRef.current;
        const c = canvasRef.current;
        if (v && c && v.readyState >= 2 && v.videoWidth > 0) {
          const w = v.videoWidth;
          const h = v.videoHeight;
          const scale = Math.min(1, DECODE_MAX_DIM / Math.max(w, h));
          const cw = Math.max(1, Math.floor(w * scale));
          const ch = Math.max(1, Math.floor(h * scale));
          if (c.width !== cw) c.width = cw;
          if (c.height !== ch) c.height = ch;
          const ctx = c.getContext("2d", { willReadFrequently: true });
          if (ctx) {
            ctx.drawImage(v, 0, 0, cw, ch);
            const img = ctx.getImageData(0, 0, cw, ch);
            const code = jsQR(img.data, cw, ch, { inversionAttempts: "attemptBoth" });
            frameCountRef.current++;
            if (code && code.data) {
              onResultRef.current(code.data);
              return; // 不再排下一帧，交由父组件卸载本面板
            }
          }
        }
        rafRef.current = requestAnimationFrame(tick);
      };
      rafRef.current = requestAnimationFrame(tick);
    };

    start();
    return () => {
      stopped = true;
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
      if (stream) stream.getTracks().forEach((tr) => tr.stop());
    };
  }, [t]);

  // 1Hz 刷新显示（帧数取自 ref，倒计时递减），到点取消
  useEffect(() => {
    const id = setInterval(() => {
      setDisplay((d) => ({
        frames: frameCountRef.current,
        seconds: Math.max(0, d.seconds - 1),
      }));
    }, 1000);
    return () => clearInterval(id);
  }, []);
  useEffect(() => {
    if (display.seconds <= 0 && !error) onCancelRef.current();
  }, [display.seconds, error]);

  return (
    <div className="fixed inset-0 z-[60] bg-black flex flex-col select-none">
      <video
        ref={videoRef}
        className="w-full h-full object-contain"
        playsInline
        muted
        autoPlay
      />
      <canvas ref={canvasRef} className="hidden" />

      {/* 顶部状态栏 */}
      <div className="absolute top-0 inset-x-0 px-4 pb-3 pt-[calc(env(safe-area-inset-top)+0.75rem)] flex items-center justify-between text-white">
        <span className="text-sm font-medium drop-shadow max-w-[70%] break-words">
          {error ?? t("space.scanningStatus", { frames: display.frames, seconds: display.seconds })}
        </span>
        <button
          type="button"
          onClick={() => onCancelRef.current()}
          className="shrink-0 px-4 py-1.5 rounded-full bg-white/20 backdrop-blur text-sm font-medium active:bg-white/30"
        >
          {t("common.cancel")}
        </button>
      </div>

      {/* 取景框（仅扫描中显示） */}
      {!error && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="w-[70vmin] h-[70vmin] max-w-[400px] max-h-[400px] rounded-xl border-2 border-white/70 shadow-[0_0_0_9999px_rgba(0,0,0,0.35)]" />
        </div>
      )}
    </div>
  );
}

function shortErr(e: unknown): string {
  const msg = String(e);
  // NotAllowedError: Permission denied / NotReadableError: camera in use
  const m = msg.match(/(\w+Error):?\s*(.*)/);
  return m ? `${m[1]}${m[2] ? `: ${m[2]}` : ""}` : msg.slice(0, 60);
}
