import { Button } from "@radix-ui/themes";
import { Home } from "lucide-react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";

export function NotFoundPage() {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-[var(--color-background)]">
      <div className="text-center space-y-4">
        <h1 className="text-6xl font-bold text-[var(--color-text-primary)]">404</h1>
        <h2 className="text-2xl font-semibold text-[var(--color-text-secondary)]">
          {t("common.pageNotFound")}
        </h2>
        <p className="text-[var(--color-text-secondary)] max-w-md mx-auto">
          {t("common.pageNotFoundDescription")}
        </p>
        <div className="flex gap-3 justify-center">
          <Link to="/">
            <Button variant="outline" size="2">
              <Home size={16} />
              {t("common.goHome")}
            </Button>
          </Link>
          <Link to="/settings">
            <Button size="2">
              {t("settings.title")}
            </Button>
          </Link>
        </div>
      </div>
    </div>
  );
}