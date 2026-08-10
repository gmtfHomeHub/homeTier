import { useEffect, useState, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Button, Flex, Text } from "@radix-ui/themes";
import { useNavigate } from "react-router-dom";
import * as api from "../../utils/api";
import { useAppTabsStore } from "../../stores/appTabsStore";
import type { SpaceApp, SystemApp, Space } from "../../types";
import { SpaceStatus } from "../../enum";
import { AppFormDialog } from "./AppFormDialog";
import { ShareAppDialog } from "./ShareAppDialog";
import { AppNavContainer, type NavApp, type NavGroup } from "./AppNavContainer";
import { toastError } from "../../utils/toast";

interface AppNavPageProps {
  space: Space;
  isOwner: boolean;
}

const SYSTEM_GROUP_KEY = "__system__";

function toNavApp(app: SpaceApp): NavApp {
  return { id: app.id, name: app.name, icon: app.icon, description: app.description, system: false };
}

function systemAppToNav(app: SystemApp, t: (key: string) => string): NavApp {
  const label = t(app.name);
  return { id: app.path, name: label !== app.name ? label : app.name, icon: app.icon, description: app.desc, enabled: app.enabled, system: true };
}

export function AppNavPage({ space, isOwner }: AppNavPageProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [apps, setApps] = useState<SpaceApp[]>([]);
  const [systemApps, setSystemApps] = useState<SystemApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [showForm, setShowForm] = useState(false);
  const [editApp, setEditApp] = useState<SpaceApp | null>(null);
  const [shareApp, setShareApp] = useState<SpaceApp | null>(null);

  const isRunning = space?.status === SpaceStatus.CED;

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const [userApps, sysApps] = await Promise.all([
        api.listApps(space.id),
        api.getSystemApps(),
      ]);
      setApps(userApps);
      setSystemApps(sysApps);
    } catch (e) {
      console.error("Failed to load apps:", e);
    } finally {
      setLoading(false);
    }
  }, [space.id]);

  useEffect(() => { loadData(); }, [loadData]);

  const handleAdd = useCallback((category?: string) => () => {
    setEditApp(category ? { category } as SpaceApp : null);
    setShowForm(true);
  }, []);

  const handleEdit = useCallback((app: NavApp) => {
    if (app.system) return;
    const found = apps.find((a) => a.id === app.id);
    if (found) {
      setEditApp(found);
      setShowForm(true);
    }
  }, [apps]);

  const handleDelete = useCallback(async (app: NavApp) => {
    if (app.system) return;
    try {
      await api.deleteApp(app.id);
      loadData();
    } catch (e) {
      toastError(String(e));
    }
  }, [loadData]);

  const handleShare = useCallback((app: NavApp) => {
    if (app.system) return;
    const found = apps.find((a) => a.id === app.id);
    if (found) setShareApp(found);
  }, [apps]);

  const handleFormSubmit = useCallback(() => {
    setShowForm(false);
    setEditApp(null);
    loadData();
  }, [loadData]);

  const handleOpen = useCallback((app: NavApp) => {
    if (!isRunning) return;
    if (editing) return;
    if (app.system) {
      navigate(`/space/${space.id}${app.id}`);
    } else {
      const found = apps.find((a) => a.id === app.id);
      if (found) {
        useAppTabsStore.getState().openApp(space, found);
        navigate(`/space/${space.id}/app/${app.id}`);
      }
    }
  }, [isRunning, editing, space, apps, navigate]);

  const groups = useMemo<NavGroup[]>(() => {
    const userGroups: NavGroup[] = [];
    const grouped = apps.reduce<Record<string, SpaceApp[]>>((acc, app) => {
      const cat = app.category || t("appNav.uncategorized");
      if (!acc[cat]) acc[cat] = [];
      acc[cat].push(app);
      return acc;
    }, {});
    for (const cat of Object.keys(grouped).sort()) {
      userGroups.push({
        title: cat,
        apps: grouped[cat].map(toNavApp),
      });
    }
    const enabledSystem = systemApps.filter((a) => a.enabled !== false);
    if (enabledSystem.length > 0) {
      userGroups.push({
        title: t("appNav.systemGroup"),
        apps: enabledSystem.map((a) => systemAppToNav(a, t)),
      });
    }
    return userGroups;
  }, [apps, systemApps, t]);

  const existingCategories = useMemo(
    () => [...new Set(apps.map((a) => a.category || t("appNav.uncategorized")))],
    [apps, t]
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-[var(--color-text-secondary)]">
        {t("common.loading")}
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1 p-4 overflow-y-auto">
      <Flex justify="between" align="center" mb="3">
        <Text size="2" weight="bold" className="text-[var(--color-text-secondary)]">
          {t("appNav.title")}
        </Text>
        {isOwner && (
          <Button
            onClick={() => setEditing(!editing)}
            variant="soft"
            size="1"
            color={editing ? "sky" : "blue"}
          >
            {editing ? t("common.done") : t("common.edit")}
          </Button>
        )}
      </Flex>

      <AppNavContainer
        groups={groups}
        mode={editing ? "edit" : "view"}
        canEdit={isOwner}
        onOpen={handleOpen}
        onEdit={handleEdit}
        onDelete={handleDelete}
        onShare={handleShare}
        onAdd={handleAdd()}
        emptyText={t("appNav.noApps")}
      />

      {showForm && (
        <AppFormDialog
          app={editApp}
          spaceId={space.id}
          existingCategories={existingCategories}
          onClose={() => setShowForm(false)}
          onSubmit={handleFormSubmit}
        />
      )}

      {shareApp && (
        <ShareAppDialog
          app={shareApp}
          currentSpaceId={space.id}
          onClose={() => setShareApp(null)}
          onShared={loadData}
        />
      )}
    </div>
  );
}
