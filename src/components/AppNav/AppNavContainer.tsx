import { useState, Fragment, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Edit3, Trash2, Share2 } from "lucide-react";
import { Button, Flex, Card, Text, Box } from "@radix-ui/themes";
import { Icon } from "@iconify/react";
import { ConfirmDialog } from "../Common/ConfirmDialog";

export interface NavApp {
  id: string;
  name: string;
  icon?: string;
  description?: string;
  enabled?: boolean;
  system?: boolean;
}

export interface NavGroup {
  title?: string;
  apps: NavApp[];
}

interface AppNavContainerProps {
  groups: NavGroup[];
  mode: "view" | "edit";
  canEdit: boolean;
  onOpen: (app: NavApp) => void;
  onEdit?: (app: NavApp) => void;
  onDelete?: (app: NavApp) => void;
  onShare?: (app: NavApp) => void;
  onAdd?: (category?: string) => void;
  emptyText?: string;
  disabled?: boolean;
}

export function AppNavContainer({
  groups,
  mode,
  canEdit,
  onOpen,
  onEdit,
  onDelete,
  onShare,
  onAdd,
  emptyText,
  disabled,
}: AppNavContainerProps) {
  const { t } = useTranslation();
  const [deleteTarget, setDeleteTarget] = useState<NavApp | null>(null);

  const allApps = groups.flatMap((g) => g.apps);
  const hasContent = allApps.length > 0;

  const handleConfirmDelete = async () => {
    if (deleteTarget && onDelete) {
      onDelete(deleteTarget);
      setDeleteTarget(null);
    }
  };

  if (!hasContent) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-[var(--color-text-secondary)]">
        {emptyText ?? t("appNav.noApps")}
        {canEdit && mode === "edit" && onAdd && (
          <Button onClick={() => onAdd()} variant="soft" size="1" className="mt-3">
            <Plus size={14} /> {t("appNav.addApp")}
          </Button>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {groups.map((group) => {
        if (group.apps.length === 0) return null;
        return (
          <div key={group.title ?? "__uncategorized"}>
            {group.title && (
              <Text
                size="1"
                weight="bold"
                className="text-[var(--color-text-secondary)] block mb-2 uppercase tracking-wider"
              >
                {group.title}
              </Text>
            )}
            <div className="grid w-full grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8">
              {group.apps.map((app, i) => (
                <Fragment key={app.id}>
                  <Box
                    className={`relative cursor-pointer group${disabled ? ' gray': ''}`}
                    onClick={() => onOpen(app)}
                  >
                    <Card>
                      <Flex gap="3" align="center">
                        {app.icon ? (
                          <Icon
                            icon={app.icon}
                            width={40}
                            height={40}
                            className="text-[var(--color-primary)]"
                          />
                        ) : (
                          <div className="w-10 h-10 rounded bg-[var(--color-border)]" />
                        )}
                        <Box className="flex-1 min-w-0">
                          <Text as="div" size="3" weight="bold" className="truncate">
                            {app.name}
                          </Text>
                          {app.description && (
                            <Text as="div" size="1" color="gray" className="truncate">
                              {app.description}
                            </Text>
                          )}
                        </Box>
                      </Flex>

                      {mode === "edit" && canEdit && !app.system && (
                        <div className="absolute flex gap-1 transition-opacity opacity-0 top-1 right-2 group-hover:opacity-100">
                          {onEdit && (
                            <Button onClick={(e) => { e.stopPropagation(); onEdit(app); }} variant="ghost" size="1">
                              <Edit3 size={12} />
                            </Button>
                          )}
                          {onShare && (
                            <Button onClick={(e) => { e.stopPropagation(); onShare(app); }} variant="ghost" size="1" title={t("common.shareApp")}>
                              <Share2 size={12} />
                            </Button>
                          )}
                          {onDelete && (
                            <Button onClick={(e) => { e.stopPropagation(); setDeleteTarget(app); }} variant="ghost" color="red" size="1">
                              <Trash2 size={12} />
                            </Button>
                          )}
                        </div>
                      )}
                    </Card>
                  </Box>
                  {mode === "edit" && onAdd && i === group.apps.length - 1 && (
                    <Box maxWidth="80px" minHeight="70px" className="relative cursor-pointer group" onClick={() => onAdd(group.title)}>
                      <Card className="flex flex-col items-center justify-center flex-1 h-full">
                        <Icon icon="icon-park-solid:add-web" width={24} height={24} />
                        <Text size="1">{t("appNav.add")}</Text>
                      </Card>
                    </Box>
                  )}
                </Fragment>
              ))}
            </div>
          </div>
        );
      })}

      {canEdit && mode === "edit" && !hasContent && onAdd && (
        <Flex justify="center" mt="2">
          <Button onClick={() => onAdd()} variant="soft" size="1">
            <Plus size={14} /> {t("appNav.addApp")}
          </Button>
        </Flex>
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onCancel={() => setDeleteTarget(null)}
        onConfirm={handleConfirmDelete}
        title={t("common.confirmDeleteApp")}
        message={t("common.confirmDeleteAppMessage", { name: deleteTarget?.name ?? "" })}
        confirmText={t("common.confirm")}
        cancelText={t("common.cancel")}
        danger
      />
    </div>
  );
}
