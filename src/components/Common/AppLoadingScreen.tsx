import { useTranslation } from "react-i18next";
import { Loading } from "./Loading";

export function AppLoadingScreen() {
  const { t } = useTranslation();
  return (
    <div className="h-screen w-screen flex flex-col items-center justify-center bg-[var(--color-bg)] gap-4">
      <Loading />
      <p className="text-sm text-[var(--color-text-secondary)]">{t("common.initializing")}</p>
    </div>
  );
}

export function AppErrorScreen({ message, onRetry }: { message: string; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="h-screen w-screen flex flex-col items-center justify-center bg-[var(--color-bg)] gap-4">
      <div className="text-4xl">⚠️</div>
      <p className="text-sm text-[var(--color-text-secondary)] max-w-xs text-center">
        {t("common.startupFailed", { message })}
      </p>
      <button
        onClick={onRetry}
        className="px-4 py-2 text-sm rounded-lg bg-[var(--color-primary)] text-white hover:opacity-90 transition-opacity"
      >
        {t("common.retry")}
      </button>
    </div>
  );
}