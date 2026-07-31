import { useEffect, useState, Fragment } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Edit3, Trash2 } from "lucide-react";
import { Button, Flex, Card, Text, Box } from "@radix-ui/themes";
import { Icon } from "@iconify/react";
import { useNavigate } from "react-router-dom";
import * as api from "../../utils/api";
import { useAppTabsStore } from "../../stores/appTabsStore";
import type { SpaceApp, Space } from "../../types";
import { SpaceStatus } from '../../enum';
import { AppFormDialog } from "./AppFormDialog";

interface AppNavPageProps {
  space: Space;
  isOwner: boolean;
  callerId: string;
}

export function AppNavPage({ space, isOwner, callerId }: AppNavPageProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [apps, setApps] = useState<SpaceApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [showForm, setShowForm] = useState(false);
  const [editApp, setEditApp] = useState<SpaceApp | null>(null);

  const isRunning = space?.status === SpaceStatus.CED;

  const loadApps = async () => {
    setLoading(true);
    try {
      const data = await api.listApps(space.id);
      setApps(data);
    } catch (e) {
      console.error("Failed to load apps:", e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadApps();
  }, [space.id]);

  const handleAdd = (category?: string) => () => {
    setEditApp(category ? { category } as SpaceApp : null);
    setShowForm(true);
  };

  const handleEdit = (app: SpaceApp) => {
    setEditApp(app);
    setShowForm(true);
  };

  const handleDelete = async (appId: string) => {
    const effectiveCallerId = callerId || space.owner_id || "";
    if (!effectiveCallerId) {
      alert(t("common.permissionError"));
      return;
    }
    if (!confirm(t("common.confirmDeleteApp"))) return;
    try {
      await api.deleteApp(appId, effectiveCallerId);
      loadApps();
    } catch (e) {
      alert(String(e));
    }
  };

  const handleFormSubmit = () => {
    setShowForm(false);
    setEditApp(null);
    loadApps();
  };

  const openApp = (app: SpaceApp) => () => {
    if (!editing && isRunning) {
      useAppTabsStore.getState().openApp(space, app);
      navigate(`/space/${space.id}/app/${app.id}`);
      return;
    }
  };

  // 按分类分组
  const grouped = apps.reduce<Record<string, SpaceApp[]>>((acc, app) => {
    const cat = app.category || "未分类";
    if (!acc[cat]) acc[cat] = [];
    acc[cat].push(app);
    return acc;
  }, {});

  const categories = Object.keys(grouped).sort();
  const existingCategories = [...new Set(apps.map((a) => a.category || "未分类"))];

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-[var(--color-text-secondary)]">
        加载中...
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1 p-4 overflow-y-auto">
      {/* 编辑模式切换 */}
      <Flex justify="between" align="center" mb="3">
        <Text size="2" weight="bold" className="text-[var(--color-text-secondary)]">
          {/* 应用导航 */}
        </Text>
        {isOwner && (
          <Button
            onClick={() => setEditing(!editing)}
            variant="soft"
            size="1"
            color={editing ? "sky" : "blue"}
          >
            {editing ? "完成" : "编辑"}
          </Button>
        )}
      </Flex>

      {apps.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-12 text-[var(--color-text-secondary)]">
          {!editing && (
            <>
              <div className="mb-2 text-4xl">📋</div>
              <Text size="2" mb="2">暂无应用</Text>
            </>
          )}
          {isOwner && editing && (
            <Button onClick={handleAdd()} variant="soft" size="1">
              <Plus size={14} /> 添加应用
            </Button>
          )}
        </div>
      ) : (
        <div className="space-y-8">
          {categories.map((cat) => (
            <div key={cat}>
              {cat !== "未分类" && (
                <Text size="1" weight="bold" className="text-[var(--color-text-secondary)] block mb-3 uppercase tracking-wider">
                  {cat}
                </Text>
              )}
              {/* 上下结构：图标在上，名称在下 */}
              <div className="grid w-full grid-cols-4 gap-3 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10" style={{ gridTemplateColumns: 'repeat(auto-fill,minmax(200px,1fr))' }}>
                {grouped[cat].map((app: SpaceApp, i) => (
                  <Fragment key={app.id}>
                    <Box className={`relative group ${isRunning ? 'cursor-pointer' : 'gray disabled'}`} onClick={openApp(app)}>
                      <Card>
                        <Flex gap="3" align="center">
                          {app.icon ? (
                            <Icon icon={app.icon} width={50} height={50} className="text-[var(--color-primary)]" />
                          ) : (
                            <div className="w-50 h-50 rounded bg-[var(--color-border)]" />
                          )}
                          <Box>
                            <Text as="div" size="5" weight="bold">
                              {app.name}
                            </Text>
                            {app.description && <Text as="div" size="2" color="gray">
                              {app.description}
                            </Text>}
                          </Box>
                        </Flex>

                        {editing && isOwner && (
                          <div className="absolute flex gap-2 transition-opacity opacity-0 top-1 right-2 group-hover:opacity-100">
                            <Button onClick={() => handleEdit(app)} variant="ghost" size="1">
                              <Edit3 size={12} />
                            </Button>
                            <Button onClick={() => handleDelete(app.id)} variant="ghost" color="red" size="1">
                              <Trash2 size={12} />
                            </Button>
                          </div>
                        )}
                      </Card>
                    </Box>
                    { editing && (i === grouped[cat].length - 1) && (
                      <Box maxWidth="70px" minHeight="70px" key={`${cat}_add`} className="relative cursor-pointer group" onClick={handleAdd(cat)}>
                        <Card className="flex flex-col items-center justify-center flex-1 h-full">
                              <Icon icon="icon-park-solid:add-web" width={24} height={24} />
                              <Text size="1">添加</Text>
                        </Card>
                      </Box>
                    )}
                  </Fragment>
                ))}
              </div>
            </div>
          ))}
          {apps.length <= 0 && editing && isOwner && (
            <Flex justify="center" mt="2">
              <Button onClick={handleAdd()} variant="soft" size="1">
                <Plus size={14} /> 添加应用
              </Button>
            </Flex>
          )}
        </div>
      )}

      {showForm && (
        <AppFormDialog
          app={editApp}
          spaceId={space.id}
          callerId={callerId}
          existingCategories={existingCategories}
          onClose={() => setShowForm(false)}
          onSubmit={handleFormSubmit}
        />
      )}
    </div>
  );
}