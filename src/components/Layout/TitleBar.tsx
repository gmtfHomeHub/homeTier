import { Menu } from "lucide-react";
import { useLayoutStore } from "../../stores/layoutStore";
import { Button } from "@radix-ui/themes";

export function TitleBar() {
  const { sidebarOpen, toggleSidebar } = useLayoutStore();

  return (
    <div
      data-tauri-drag-region
      className="h-10 flex items-center gap-3 px-3 bg-[var(--color-surface)] border-b border-[var(--color-border)] select-none"
    >
      <Button
        onClick={toggleSidebar}
        variant="ghost"
        size="2"
        className="shrink-0"
        title={sidebarOpen ? "收起侧栏" : "展开侧栏"}
      >
        <Menu size={18} />
      </Button>
      {/* <div className="flex items-center gap-2" data-tauri-drag-region>
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-[var(--color-primary)]"
        >
          <path d="M12 2L2 7l10 5 10-5-10-5z" />
          <path d="M2 17l10 5 10-5" />
          <path d="M2 12l10 5 10-5" />
        </svg>
        <span className="text-sm font-semibold text-[var(--color-text-secondary)]">
          homeTier
        </span>
      </div> */}
    </div>
  );
}