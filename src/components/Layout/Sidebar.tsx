import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { useNavigate, useLocation } from "react-router-dom";
import { MessageSquare, LogIn, Settings, Plus, House } from "lucide-react";
import { useState } from "react";
import { CreateSpaceDialog } from "../Space/CreateSpaceDialog";
import { JoinSpaceDialog } from "../Space/JoinSpaceDialog";
import { Button, Badge, Flex } from "@radix-ui/themes";
import { useLayoutStore } from "../../stores/layoutStore";
import { useSwipe } from "../../hooks/useSwipe";
import { Loading } from "../Common/Loading";

export function Sidebar() {
  const { t } = useTranslation();
  const { spaces, loading, error, isReady, setCurrentSpace } = useSpaceStore();
  const navigate = useNavigate();
  const location = useLocation();
  const [showCreate, setShowCreate] = useState(false);
  const [showJoin, setShowJoin] = useState(false);
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
        className="w-64 bg-[var(--color-surface)] border-r border-[var(--color-border)] flex flex-col h-full relative"
        {...swipeBind}
      >
        <div
          className="absolute top-0 bottom-0 right-0 z-20 hidden w-4 cursor-w-resize md:block"
          onClick={() => setSidebarOpen(false)}
        />

        <div className="flex-1 p-3 overflow-x-hidden overflow-y-auto">
          <div className="flex items-center justify-between mb-3">
            <Flex gap="2">
              <Button
                onClick={() => navigate("/")}
                variant="ghost"
                size="2"
                className="flex-1"
              >
                <Flex align="center" gap="2">
                  <House size={16} />
                  {t("space.spaces")}
                </Flex>
              </Button>
            </Flex>
            <Flex gap="3">
              <Button
                onClick={() => setShowJoin(true)}
                variant="ghost"
                size="2"
                title={t("space.join")}
              >
                <LogIn size={16} />
              </Button>
              <Button
                onClick={() => setShowCreate(true)}
                variant="ghost"
                size="2"
                title={t("space.create")}
              >
                <Plus size={16} />
              </Button>
            </Flex>
          </div>

          {loading && !isReady && (
            <div className="py-8"><Loading /></div>
          )}
          {!loading && error && !isReady && (
            <div className="text-center py-8 text-[var(--color-text-secondary)] text-sm">
              ⚠️ {t("settings.loadError") ?? "服务未就绪"}
            </div>
          )}
          {!loading && !error && spaces.length === 0 && (
            <div className="text-center py-8 text-[var(--color-text-secondary)] text-sm">
              {t("space.noSpaces")}
              <br />
              <Button
                onClick={() => setShowCreate(true)}
                variant="ghost"
                size="1"
                className="text-[var(--color-primary)]"
              >
                {t("space.createFirstSpace")}
              </Button>
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

        <div className="p-3 border-t border-[var(--color-border)]">
          <Flex gap="2">
            <Button onClick={() => navigate("/")} variant="ghost" size="2" className="flex-1">
              <Flex align="center" gap="2" justify="center">
                <MessageSquare size={16} />
                <span>{t("chat.title")}</span>
              </Flex>
            </Button>
            <Button onClick={() => navigate("/settings")} variant="ghost" size="2" className="flex-1">
              <Flex align="center" gap="2" justify="center">
                <Settings size={16} />
                <span>{t("settings.title")}</span>
              </Flex>
            </Button>
          </Flex>
        </div>
      </aside>

      {showCreate && <CreateSpaceDialog onClose={() => setShowCreate(false)} />}
      {showJoin && <JoinSpaceDialog onClose={() => setShowJoin(false)} />}
    </>
  );
}
