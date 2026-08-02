import { useSpaceStore } from "../../stores/spaceStore";
import { useSpaceConnect } from "../../hooks/useSpaceConnect";
import { useNavigate } from "react-router-dom";
import { Share2, Trash2, Settings, X, LogIn, Plus, Ellipsis, House } from "lucide-react";
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ShareSpaceDialog } from "../Common/ShareSpaceDialog";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { EasyTierConfigEditor } from "../Network/EasyTierConfigEditor";
import { MemberCount } from "../Common/MemberCount";
import { Button, Flex, Grid, Badge, DropdownMenu } from "@radix-ui/themes";
import { getSystemConfig, updateSpaceConfig } from "../../utils/api";
import { toastError } from "../../utils/toast";
import type { NetworkConfig } from "../../types/network";
import { DEFAULT_NETWORK_CONFIG } from "../../types/network";
import { getSpaceIp, handleStopProp } from "../../utils";
import { CreateSpaceDialog } from "../Space/CreateSpaceDialog";
import { JoinSpaceDialog } from "../Space/JoinSpaceDialog";

export function SpaceList() {
  const { spaces, deleteSpace, loadSpaces, loadSpacesOnce } = useSpaceStore();
  const { connectingId, disconnectingId, connect, disconnect } = useSpaceConnect();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
  const [configTarget, setConfigTarget] = useState<string | null>(null);
  const [spaceConfig, setSpaceConfig] = useState<Partial<NetworkConfig>>({});
  const [savingConfig, setSavingConfig] = useState(false);
  const [shareTarget, setShareTarget] = useState<string | null>(null);

  const [showCreate, setShowCreate] = useState(false);
  const [showJoin, setShowJoin] = useState(false);

  const configSpace = spaces.find(s => s.id === configTarget);

  useEffect(() => {
    if (configTarget && configSpace) {
      let spaceCfg: Partial<NetworkConfig> = {};
      if (configSpace.config_json) {
        try { spaceCfg = JSON.parse(configSpace.config_json); } catch (err) {
          console.log(err);
        }
      }

      Promise.all([
        getSystemConfig(),
      ]).then(([sysJson]) => {
        let merged: Partial<NetworkConfig> = {};

        if (sysJson) {
          try { merged = JSON.parse(sysJson); } catch (err) {
            console.log(err);
          }
        }

        merged = { ...merged, ...spaceCfg };

        if (!merged.network_name) {
          merged.network_name = configSpace.network_name;
        }
        if (!merged.network_secret) {
          merged.network_secret = configSpace.network_secret;
        }

        setSpaceConfig(merged);
      });
    }
  }, [configTarget]);

  const handleSaveConfig = async () => {
    if (!configTarget) return;
    setSavingConfig(true);
    try {
      await updateSpaceConfig(configTarget, JSON.stringify({ ...DEFAULT_NETWORK_CONFIG(), ...spaceConfig }));
      await loadSpaces();
      setConfigTarget(null);
    } catch (e) {
      toastError(String(e));
    } finally {
      setSavingConfig(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteSpace(deleteTarget.id);
      await new Promise((r) => setTimeout(r, 300));
      await loadSpacesOnce();
      setDeleteTarget(null);
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <div className="flex-1 p-6 overflow-y-auto">
      <Flex align="center" justify="between" className="mb-6">
        <h1 className="text-2xl font-bold">{t("space.list")}</h1>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            <Button variant="ghost" size="1" className="py-2.5 pb-[9px]">
              <Ellipsis size={16} />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Content>
            <DropdownMenu.Item onClick={() => setShowCreate(true)}>
              <Plus size={16} />
              {t("space.create")}
            </DropdownMenu.Item>
            <DropdownMenu.Item onClick={() => setShowJoin(true)}>
              <LogIn size={16} />
              {t("space.join")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </Flex>
      {spaces.length === 0 && (
        <div className="text-center py-20 text-[var(--color-text-secondary)]">
          <div className="mb-4 text-5xl">🏠</div>
          <p className="mb-2 text-lg">{t("space.notJoined")}</p>
          <p className="text-sm">{t("space.createOrJoinHint")}</p>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
        {spaces.map((space) => (
          <div
            key={space.id}
            onClick={() => navigate(`/space/${space.id}`)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                navigate(`/space/${space.id}`);
              }
            }}
            className="bg-[var(--color-surface)] rounded-xl p-5 border border-[var(--color-border)] hover:shadow-md transition-shadow cursor-pointer"
          >
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <div
                  className={`w-3 h-3 rounded-full ${
                    space.status === "connected"
                      ? "bg-[var(--color-success)]"
                      : space.status === "connecting"
                        ? "bg-yellow-400 animate-pulse"
                        : "bg-[var(--color-text-secondary)]"
                  }`}
                />
                <h3 className="font-semibold truncate">{space.name}</h3>
                {getSpaceIp(space) && (
                  <Badge
                    color="gray"
                    variant="soft"
                    size="1"
                    className="mt-0.5 font-mono max-w-full truncate"
                    title={getSpaceIp(space) || ""}
                  >
                    {getSpaceIp(space)}
                  </Badge>
                )}
              </div>
              <div
                className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]"
                onClick={(e) => e.stopPropagation()}
              >
                <MemberCount
                  spaceId={space.id}
                  connected={space.status === "connected"}
                />
              </div>
            </div>
            <Grid columns={{ initial: "1", sm: "2" }} gap="3">
              <Flex>
                {space.status === "connected" ? (
                  <Button
                    onClick={handleStopProp(() => disconnect(space.id))}
                    disabled={disconnectingId === space.id}
                    loading={disconnectingId === space.id}
                    variant="outline"
                    size="2"
                  >
                    {disconnectingId === space.id
                      ? t("space.disconnecting")
                      : t("space.disconnect")}
                  </Button>
                ) : (
                  <Button
                    onClick={handleStopProp(() => connect(space.id))}
                    disabled={
                      space.status === "connecting" || connectingId === space.id
                    }
                    variant="soft"
                    size="2"
                    loading={
                      space.status === "connecting" || connectingId === space.id
                    }
                  >
                    {space.status === "connecting"
                      ? t("space.connecting")
                      : t("space.connect")}
                  </Button>
                )}
              </Flex>
              <Flex align="center" justify="end" gap="4">
                <Button
                  onClick={handleStopProp(() => setShareTarget(space.id))}
                  variant="ghost"
                  size="2"
                  title={t("space.share")}
                >
                  <Share2 size={16} />
                </Button>
                <Button
                  onClick={handleStopProp(() => setConfigTarget(space.id))}
                  variant="ghost"
                  size="2"
                  title={t("space.spaceConfig")}
                >
                  <Settings size={16} />
                </Button>
                {space.owner_id && (
                  <Button
                    onClick={handleStopProp(() =>
                      setDeleteTarget({
                        id: space.id,
                        name: space.name,
                      }),
                    )}
                    variant="ghost"
                    color="red"
                    size="2"
                    title={t("space.deleteSpace")}
                  >
                    <Trash2 size={16} />
                  </Button>
                )}
              </Flex>
            </Grid>
          </div>
        ))}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title={t("space.deleteSpace")}
        message={t("space.confirmDeleteMessage", {
          name: deleteTarget?.name || "",
        })}
        confirmText={t("space.delete")}
        danger
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />

      {configTarget && configSpace && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[var(--color-surface)] rounded-xl w-full max-w-[calc(100vw-24px)] sm:w-[640px] max-h-[80vh] flex flex-col shadow-xl">
            <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
              <h2 className="text-lg font-semibold">
                {t("space.spaceConfigTitle", { name: configSpace.name })}
              </h2>
              <Button
                onClick={() => setConfigTarget(null)}
                variant="ghost"
                size="2"
              >
                <X size={20} />
              </Button>
            </div>
            <div className="flex-1 p-6 overflow-y-auto">
              <EasyTierConfigEditor
                value={spaceConfig}
                onChange={setSpaceConfig}
                title={t("space.spaceConfigSubtitle")}
              />
            </div>
            <Flex
              justify="end"
              gap="2"
              px="6"
              py="4"
              className="border-t border-[var(--color-border)]"
            >
              <Button
                onClick={() => setConfigTarget(null)}
                variant="outline"
                size="2"
              >
                {t("common.cancel")}
              </Button>
              <Button
                onClick={handleSaveConfig}
                disabled={savingConfig}
                variant="solid"
                color="blue"
                size="2"
                loading={savingConfig}
              >
                {savingConfig ? t("common.saving") : t("space.saveConfig")}
              </Button>
            </Flex>
          </div>
        </div>
      )}

      {shareTarget && (
        <ShareSpaceDialog
          spaceId={shareTarget}
          onClose={() => setShareTarget(null)}
        />
      )}

      {showCreate && <CreateSpaceDialog onClose={() => setShowCreate(false)} />}
      {showJoin && <JoinSpaceDialog onClose={() => setShowJoin(false)} />}
    </div>
  );
}
