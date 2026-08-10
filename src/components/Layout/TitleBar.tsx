import { Menu, Settings, House } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Button, Grid, Flex, Separator } from "@radix-ui/themes";
import { useLayoutStore } from "../../stores/layoutStore";

export function TitleBar() {
  const { t } = useTranslation();
  const { sidebarOpen, toggleSidebar } = useLayoutStore();
  const navigate = useNavigate();

  return (
    <div
      data-tauri-drag-region
      className="h-10 flex items-center gap-3 px-3 bg-[var(--color-surface)] border-b border-[var(--color-border)] select-none relative z-35"
    >
      <Grid columns="3" gap="3" className="w-full">
        <Flex align="center">
          <Button
            onClick={toggleSidebar}
            variant="ghost"
            size="2"
            className="shrink-0"
            title={sidebarOpen ? t("common.collapseSidebar") : t("common.expandSidebar")}
          >
            <Menu size={18} />
          </Button>
        </Flex>
        <div />
        <div>
          <Flex
            gap="3"
            className="flex items-center justify-end"
            // data-tauri-drag-region
          >
            <Button onClick={() => navigate("/")} variant="ghost" size="1">
              <Flex align="center" gap="1">
                <House size={16} />
                <span className="hidden sm:inline">{t("space.spaces")}</span>
              </Flex>
            </Button>
            <Separator orientation="vertical" className="hidden sm:block" />
            <Button
              onClick={() => navigate("/settings")}
              variant="ghost"
              size="1"
            >
              <Flex align="center" gap="1" justify="center">
                <Settings size={16} />
                <span className="hidden sm:inline">{t("settings.title")}</span>
              </Flex>
            </Button>
          </Flex>
        </div>
      </Grid>
    </div>
  );
}
