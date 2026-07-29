import { useState } from "react";
import { Menu, Settings, House, LogIn, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Button, Grid, Flex, DropdownMenu } from "@radix-ui/themes";
import { useLayoutStore } from "../../stores/layoutStore";
import { CreateSpaceDialog } from "../Space/CreateSpaceDialog";
import { JoinSpaceDialog } from "../Space/JoinSpaceDialog";

export function TitleBar() {
  const { t } = useTranslation();
  const { sidebarOpen, toggleSidebar } = useLayoutStore();
  const navigate = useNavigate();
  const [showCreate, setShowCreate] = useState(false);
  const [showJoin, setShowJoin] = useState(false);

  return (
    <div
      data-tauri-drag-region
      className="h-10 flex items-center gap-3 px-3 bg-[var(--color-surface)] border-b border-[var(--color-border)] select-none relative z-35"
    >
      <Grid columns="3" gap="3" className="w-full">
        <div>
          <Button
            onClick={toggleSidebar}
            variant="ghost"
            size="2"
            className="shrink-0"
            title={sidebarOpen ? "收起侧栏" : "展开侧栏"}
          >
            <Menu size={18} />
          </Button>
        </div>
        <div />
        <div>
          <Flex
            gap="4"
            className="flex items-center justify-end"
            // data-tauri-drag-region
          >
            <DropdownMenu.Root>
                <Button onClick={() => navigate("/")} variant="ghost" size="2">
                  <Flex align="center" gap="2">
                    <House size={16} />
                    {t("space.spaces")}
                  </Flex>
                </Button>
              <DropdownMenu.Trigger>
                <Button variant="ghost" size="1" className="py-2.5 pb-[9px]">
                <DropdownMenu.TriggerIcon />
                </Button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Content>
                <DropdownMenu.Item
                  onClick={() => setShowCreate(true)}
                  shortcut={t("space.create")}
                >
                  <Plus size={16} />
                </DropdownMenu.Item>
                <DropdownMenu.Item
                  onClick={() => setShowJoin(true)}
                  shortcut={t("space.join")}
                >
                  <LogIn size={16} />
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
            <Button
              onClick={() => navigate("/settings")}
              variant="ghost"
              size="2"
            >
              <Flex align="center" gap="2" justify="center">
                <Settings size={16} />
                <span>{t("settings.title")}</span>
              </Flex>
            </Button>
          </Flex>
        </div>

        {showCreate && (
          <CreateSpaceDialog onClose={() => setShowCreate(false)} />
        )}
        {showJoin && <JoinSpaceDialog onClose={() => setShowJoin(false)} />}
      </Grid>
    </div>
  );
}
