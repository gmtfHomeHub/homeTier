import { useEffect, useState } from "react";
import { Plus, Edit3, Trash2 } from "lucide-react";
import { Button, Flex, Card, Text } from "@radix-ui/themes";
import { Icon } from "@iconify/react";
import { useNavigate } from "react-router-dom";
import * as api from "../../utils/api";
import type { SpaceApp, Space } from "../../types";
import { AppFormDialog } from "./AppFormDialog";

interface AppNavPageProps {
  space: Space;
  isOwner: boolean;
  callerId: string;
}

export function AppNavPage({ space, isOwner, callerId }: AppNavPageProps) {
  const navigate = useNavigate();
  const [apps, setApps] = useState<SpaceApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [showForm, setShowForm] = useState(false);
  const [editApp, setEditApp] = useState<SpaceApp | null>(null);

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

  const handleAdd = () => {
    setEditApp(null);
    setShowForm(true);
  };

  const handleEdit = (app: SpaceApp) => {
    setEditApp(app);
    setShowForm(true);
  };

  const handleDelete = async (appId: string) => {
    const effectiveCallerId = callerId || space.owner_id || "";
    if (!effectiveCallerId) {
      alert("无法获取权限信息");
      return;
    }
    if (!confirm("确定要删除此应用吗？")) return;
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
          应用导航
        </Text>
        {isOwner && (
          <Button
            onClick={() => setEditing(!editing)}
            variant="soft"
            size="1"
            color={editing ? "blue" : "gray"}
          >
            {editing ? "完成" : "编辑"}
          </Button>
        )}
      </Flex>

      {apps.length === 0 && !editing ? (
        <div className="flex flex-col items-center justify-center py-12 text-[var(--color-text-secondary)]">
          <div className="text-4xl mb-2">📋</div>
          <Text size="2" mb="2">暂无应用</Text>
          {isOwner && (
            <Button onClick={handleAdd} variant="soft" size="1">
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
              <div className="grid grid-cols-4 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 gap-3">
                {grouped[cat].map((app) => (
                  <Card key={app.id} className="relative group cursor-pointer" onClick={() => navigate(`/space/${space.id}/app/${app.id}`)}>
                    <div
                      className="flex flex-col items-center gap-2 p-3 text-center"
                    >
                      {/* 上部：图标 */}
                      <div className="w-10 h-10 rounded-xl bg-[var(--color-primary)]/10 flex items-center justify-center">
                        {app.icon ? (
                          <Icon icon={app.icon} width={24} height={24} className="text-[var(--color-primary)]" />
                        ) : (
                          <div className="w-5 h-5 rounded bg-[var(--color-border)]" />
                        )}
                      </div>
                      {/* 下部：名称 */}
                      <Text size="1" className="truncate w-full text-[var(--color-text)]">
                        {app.name}
                      </Text>
                    </div>
                    {editing && isOwner && (
                      <div className="absolute top-1 right-1 flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                        <Button onClick={() => handleEdit(app)} variant="ghost" size="1">
                          <Edit3 size={12} />
                        </Button>
                        <Button onClick={() => handleDelete(app.id)} variant="ghost" color="red" size="1">
                          <Trash2 size={12} />
                        </Button>
                      </div>
                    )}
                  </Card>
                ))}
                {editing && isOwner && cat === categories[categories.length - 1] && (
                  <Card className="border-dashed cursor-pointer hover:bg-[var(--color-border)] transition-colors">
                    <button
                      onClick={handleAdd}
                      className="flex flex-col items-center gap-2 p-3 w-full h-full"
                    >
                      <div className="w-10 h-10 rounded-xl bg-[var(--color-border)] flex items-center justify-center">
                        <Plus size={20} className="text-[var(--color-text-secondary)]" />
                      </div>
                      <Text size="1" className="text-[var(--color-text-secondary)]">
                        添加应用
                      </Text>
                    </button>
                  </Card>
                )}
              </div>
            </div>
          ))}
          {apps.length > 0 && editing && isOwner && (
            <Flex justify="center" mt="2">
              <Button onClick={handleAdd} variant="soft" size="1">
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