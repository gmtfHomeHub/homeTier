import { useState } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
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
            {isEditing ? t("appNav.editApp") : t("appNav.addApp")}
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
              {t("appNav.appName")}
            </Text>
            <TextField.Root
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("appNav.appNamePlaceholder")}
              autoFocus
            />
          </div>

          {/* 分类 — 下拉选择 + 输入新模式 */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              {t("appNav.category")}
            </Text>
            {categoryMode === "select" ? (
              <Select.Root value={category} onValueChange={handleCategorySelect}>
                <Select.Trigger className="w-full" />
                <Select.Content>
                  {existingCategories.map((cat) => (
                    <Select.Item key={cat} value={cat}>
                      {cat || t("appNav.uncategorized")}
                    </Select.Item>
                  ))}
                  <Select.Item value={NEW_CATEGORY_VALUE}>
                    {t("appNav.newCategory")}
                  </Select.Item>
                </Select.Content>
              </Select.Root>
            ) : (
              <Flex gap="2">
                <TextField.Root
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  placeholder={t("appNav.newCategoryPlaceholder")}
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
                    {t("appNav.selectExisting")}
                  </Button>
                )}
              </Flex>
            )}
          </div>

          {/* 图标 — Iconify */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              {t("appNav.icon")}
            </Text>
            <Flex gap="2">
              <TextField.Root
                value={icon}
                onChange={(e) => setIcon(e.target.value)}
                placeholder={t("appNav.iconPlaceholder")}
                className="flex-1"
              />
              <Button type="button" onClick={openIconSearch} variant="ghost" size="2" title={t("appNav.searchIcon")}>
                <Search size={16} />
              </Button>
            </Flex>
            <Text size="1" className="text-[var(--color-text-secondary)] mt-1">
              <a href="https://icon-sets.iconify.design/" target="_blank" rel="noopener noreferrer" className="underline">{t("appNav.fromIconify")}</a> {t("appNav.searchIconHint")}
            </Text>
          </div>

          {/* URL 分段 */}
          <div>
            <Text as="label" size="2" weight="medium" mb="1" className="block">
              {t("appNav.address")}
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
                placeholder={t("appNav.hostnamePlaceholder")}
                className="flex-1"
              />
              <Text size="1" className="text-[var(--color-text-secondary)]">:</Text>
              <TextField.Root
                value={port}
                onChange={(e) => setPort(e.target.value)}
                placeholder={t("appNav.portPlaceholder")}
                style={{ width: 80 }}
              />
              <Text size="1" className="text-[var(--color-text-secondary)]">/</Text>
              <TextField.Root
                value={pathname}
                onChange={(e) => setPathname(e.target.value)}
                placeholder={t("appNav.pathPlaceholder")}
                className="flex-1"
              />
            </Flex>
            {hostname && (
              <Text size="1" className="text-[var(--color-text-secondary)] mt-1 block">
                {t("appNav.preview")} {urlPreview}
              </Text>
            )}
          </div>

          <Flex justify="end" gap="2" pt="2">
            <Button type="button" onClick={onClose} variant="outline" size="2">
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={saving || !name.trim()} variant="solid" color="blue" size="2" loading={saving}>
              {saving ? t("common.saving") : isEditing ? t("common.save") : t("common.add")}
            </Button>
          </Flex>
        </form>
      </Dialog.Content>
    </Dialog.Root>
  );
}