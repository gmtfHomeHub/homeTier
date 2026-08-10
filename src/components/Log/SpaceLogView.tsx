import { useParams } from "react-router-dom";
import { LogViewer } from "../Log/LogViewer";
import { ArrowLeft, Terminal } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useSpaceStore } from "../../stores/spaceStore";
import { Button } from "@radix-ui/themes";
import { useTranslation } from "react-i18next";

export function SpaceLogView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const spaces = useSpaceStore((s) => s.spaces);
  const space = spaces.find((s) => s.id === id);
  const { t } = useTranslation();

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* 头部 */}
      <div className="h-14 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)]">
        <Button
          onClick={() => navigate(`/space/${id}`)}
          variant="ghost"
          size="2"
        >
          <ArrowLeft size={20} />
        </Button>
        <Terminal size={18} className="text-[var(--color-text-secondary)]" />
        <span className="font-semibold">
          {space ? t("space.logTitle", { name: space.name }) : t("space.logTitleFallback")}
        </span>
      </div>

      {/* 日志内容 */}
      <div className="flex-1 flex flex-col min-h-0">
        <LogViewer spaceId={id} />
      </div>
    </div>
  );
}