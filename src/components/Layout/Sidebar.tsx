import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { useNavigate, useLocation } from "react-router-dom";
import { Button, Badge } from "@radix-ui/themes";
import { useLayoutStore } from "../../stores/layoutStore";
import { useSwipe } from "../../hooks/useSwipe";

export function Sidebar() {
  const { t } = useTranslation();
  const { spaces, setCurrentSpace } = useSpaceStore();
  const navigate = useNavigate();
  const location = useLocation();
  const { setSidebarOpen } = useLayoutStore();

  const handleSpaceClick = (spaceId: string) => {
    setCurrentSpace(spaceId);
    navigate(`/space/${spaceId}`);
  };

  const getConfigIp = (configJson?: string): string | undefined => {
    if (!configJson) return undefined;
    try {
      const parsed = JSON.parse(configJson);
      return parsed.virtual_ipv4 || undefined;
    } catch {
      return undefined;
    }
  };

  const swipeBind = useSwipe({
    onSwipeLeft: () => setSidebarOpen(false),
  });

  return (
    <>
      <aside
        className="relative flex flex-col h-full"
        {...swipeBind}
      >
        <div className="flex-1 p-3 overflow-x-hidden overflow-y-auto">
          {spaces.length === 0 && (
            <div className="text-center py-8 text-[var(--color-text-secondary)] text-sm">
              {t("space.noSpaces")}
            </div>
          )}

          <div className="w-full space-y-1">
            {spaces.map((space) => (
              <Button
                key={space.id}
                onClick={() => handleSpaceClick(space.id)}
                variant="ghost"
                size="2"
                className={`w-full justify-start gap-3 px-3 py-2.5 text-left hover:bg-[var(--color-border)] ${
                  location.pathname.includes(space.id)
                    ? "text-[var(--color-primary)]"
                    : ""
                }`}
              >
                <div
                  className={`w-2 h-2 rounded-full shrink-0 ${
                    space.status === "connected"
                      ? "bg-[var(--color-success)]"
                      : space.status === "connecting"
                      ? "bg-yellow-400 animate-pulse"
                      : "bg-[var(--color-text-secondary)]"
                  }`}
                />
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate" title={space.name}>{space.name}</div>
                  {(space.virtual_ip || getConfigIp(space.config_json)) && (
                    <Badge color="gray" variant="soft" size="1" className="mt-0.5 font-mono max-w-full truncate" title={space.virtual_ip || getConfigIp(space.config_json) || ""}>
                      {space.virtual_ip || getConfigIp(space.config_json)}
                    </Badge>
                  )}
                </div>
              </Button>
            ))}
          </div>
        </div>
      </aside>
    </>
  );
}
