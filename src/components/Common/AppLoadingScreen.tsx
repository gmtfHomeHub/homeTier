import { Loading } from "./Loading";

export function AppLoadingScreen() {
  return (
    <div className="h-screen w-screen flex flex-col items-center justify-center bg-[var(--color-bg)] gap-4">
      <Loading />
      <p className="text-sm text-[var(--color-text-secondary)]">服务初始化中...</p>
    </div>
  );
}

export function AppErrorScreen({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="h-screen w-screen flex flex-col items-center justify-center bg-[var(--color-bg)] gap-4">
      <div className="text-4xl">⚠️</div>
      <p className="text-sm text-[var(--color-text-secondary)] max-w-xs text-center">
        服务启动失败：{message}
      </p>
      <button
        onClick={onRetry}
        className="px-4 py-2 text-sm rounded-lg bg-[var(--color-primary)] text-white hover:opacity-90 transition-opacity"
      >
        重试
      </button>
    </div>
  );
}