import { useState } from "react";
import { X, Search } from "lucide-react";
import { Button, TextField, Select, Flex, Text, Dialog } from "@radix-ui/themes";
import { Icon } from "@iconify/react";
import * as api from "../../utils/api";
import type { SpaceApp } from "../../types";
import { toastError } from "../../utils/toast";

interface AppFormDialogProps {
  app: SpaceApp | null;
  spaceId: string;
  existingCategories: string[];
  onClose: () => void;
  onSubmit: () => void;
}

const PROTOCOL_OPTIONS = [
  { value: "http:", label: "http:" },
  { value: "https:", label: "https:" },
];

const NEW_CATEGORY_VALUE = "__new__";

export function AppFormDialog({ app, spaceId, existingCategories, onClose, onSubmit }: AppFormDialogProps) {
  const isEditing = !!app;
  const [name, setName] = useState(app?.name ?? "");
  const [category, setCategory] = useState(app?.category ?? "");
  const [categoryMode, setCategoryMode] = useState<"select" | "input">(
    app?.category && existingCategories.includes(app.category) ? "select" : "input"
  );
  const [icon, setIcon] = useState(app?.icon ?? "");
  const [protocol, setProtocol] = useState(app?.protocol ?? "http:");
  const [hostname, setHostname] = useState(app?.hostname ?? "");
  const [port, setPort] = useState(app?.port ?? "");
  const [pathname, setPathname] = useState(app?.pathname ?? "");
  const [saving, setSaving] = useState(false);

  const urlPreview = `${protocol}//${hostname}${port ? `:${port}` : ""}${pathname ? `/${pathname.replace(/^\//, "")}` : ""}`;

  const handleCategorySelect = (val: string) => {
    if (val === NEW_CATEGORY_VALUE) {
      setCategoryMode("input");
      setCategory("");
    } else {
      setCategory(val);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setSaving(true);
    try {
      const options = {
        category: category || undefined,
        icon: icon || undefined,
        protocol,
        hostname: hostname || undefined,
        port: port || undefined,
        pathname: pathname || undefined,
      };

      if (isEditing && app && app.id) {
        await api.updateApp(app.id, name.trim(), options);
      } else {
        await api.addApp(spaceId, name.trim(), options);
      }
      onSubmit();
    } catch (e) {
      toastError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const openIconSearch = () => {
    window.open("https://icon-sets.iconify.design/", "_blank");
  };

  return (
    <Dialog.Root open={true} onOpenChange={() => onClose()}>
      <Dialog.Content className="w-full max-w-[calc(100vw-24px)] sm:w-[520px]">
        <div className="flex items-center justify-between mb-4">
          <Dialog.Title className="m-0 text-lg font-semibold">
            {isEditing ? "编辑应用" : "添加应用"}
          </Dialog.Title>
          <Dialog.Close>
            <Button variant="ghost" size="2">
              <X size={20} />
            </Button>
          </Dialog.Close>
        </div>

        {/* 应用预览 */}
        <div className="flex justify-center mb-6">
          <div className="w-[70px] h-[70px] rounded-xl bg-[var(--color-border)] flex items-center justify-center overflow-hidden">
            {icon ? (
              <Icon icon={icon} width={48} height={48} />
            ) : (
              <div className="w-10 h-10 rounded-lg bg-[var(--color-text-secondary)]/10" />
            )}
          </div>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* 名称 */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              应用名称
            </Text>
            <TextField.Root
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="输入应用名称"
              autoFocus
            />
          </div>

          {/* 分类 — 下拉选择 + 输入新模式 */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              分类
            </Text>
            {categoryMode === "select" ? (
              <Select.Root value={category} onValueChange={handleCategorySelect}>
                <Select.Trigger className="w-full" />
                <Select.Content>
                  {existingCategories.map((cat) => (
                    <Select.Item key={cat} value={cat}>
                      {cat || "未分类"}
                    </Select.Item>
                  ))}
                  <Select.Item value={NEW_CATEGORY_VALUE}>
                    + 输入新分类
                  </Select.Item>
                </Select.Content>
              </Select.Root>
            ) : (
              <Flex gap="2">
                <TextField.Root
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  placeholder="输入新分类名称"
                  className="flex-1"
                />
                {existingCategories.length > 0 && (
                  <Button
                    type="button"
                    onClick={() => {
                      setCategoryMode("select");
                      setCategory(existingCategories[0]);
                    }}
                    variant="ghost"
                    size="2"
                  >
                    选择已有
                  </Button>
                )}
              </Flex>
            )}
          </div>

          {/* 图标 — Iconify */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              图标（Iconify 名称）
            </Text>
            <Flex gap="2">
              <TextField.Root
                value={icon}
                onChange={(e) => setIcon(e.target.value)}
                placeholder="如: mdi:home, ph:globe"
                className="flex-1"
              />
              <Button type="button" onClick={openIconSearch} variant="ghost" size="2" title="搜索图标">
                <Search size={16} />
              </Button>
            </Flex>
            <Text size="1" className="text-[var(--color-text-secondary)] mt-1">
              从 <a href="https://icon-sets.iconify.design/" target="_blank" rel="noopener noreferrer" className="underline">icon-sets.iconify.design</a> 搜索图标，复制名称后粘贴
            </Text>
          </div>

          {/* URL 分段 */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              地址
            </Text>
            <Flex gap="2" align="end">
              <Select.Root value={protocol} onValueChange={setProtocol}>
                <Select.Trigger className="w-24" />
                <Select.Content>
                  {PROTOCOL_OPTIONS.map((opt) => (
                    <Select.Item key={opt.value} value={opt.value}>
                      {opt.label}
                    </Select.Item>
                  ))}
                </Select.Content>
              </Select.Root>
              <Text size="1" className="text-[var(--color-text-secondary)]">//</Text>
              <TextField.Root
                value={hostname}
                onChange={(e) => setHostname(e.target.value)}
                placeholder="主机地址"
                className="flex-1"
              />
              <Text size="1" className="text-[var(--color-text-secondary)]">:</Text>
              <TextField.Root
                value={port}
                onChange={(e) => setPort(e.target.value)}
                placeholder="端口"
                style={{ width: 80 }}
              />
              <Text size="1" className="text-[var(--color-text-secondary)]">/</Text>
              <TextField.Root
                value={pathname}
                onChange={(e) => setPathname(e.target.value)}
                placeholder="路径"
                className="flex-1"
              />
            </Flex>
            {hostname && (
              <Text size="1" className="text-[var(--color-text-secondary)] mt-1 block">
                预览: {urlPreview}
              </Text>
            )}
          </div>

          <Flex justify="end" gap="2" pt="2">
            <Button type="button" onClick={onClose} variant="outline" size="2">
              取消
            </Button>
            <Button type="submit" disabled={saving || !name.trim()} variant="solid" color="blue" size="2" loading={saving}>
              {saving ? "保存中..." : isEditing ? "保存" : "添加"}
            </Button>
          </Flex>
        </form>
      </Dialog.Content>
    </Dialog.Root>
  );
}