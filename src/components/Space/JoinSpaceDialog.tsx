import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { resolveJoinShareInfo } from "../../utils/share";
import type { ShareInfo } from "../../types";
import { X } from "lucide-react";
import { Button, TextField, Flex } from "@radix-ui/themes";
import { toastError } from "../../utils/toast";
import { readText } from "@tauri-apps/plugin-clipboard-manager";

interface JoinSpaceDialogProps {
  /** 由外部（如 SpaceList 扫一扫分发器）预先解析好的 ShareInfo，传入即直接进入确认态 */
  initialShare?: ShareInfo;
  onClose: () => void;
}

export function JoinSpaceDialog({ initialShare, onClose }: JoinSpaceDialogProps) {
  const { t } = useTranslation();
  const [networkName, setNetworkName] = useState("");
  const [networkSecret, setNetworkSecret] = useState("");
  const [pendingShare, setPendingShare] = useState<ShareInfo | null>(
    initialShare ?? null
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pastedLink, setPastedLink] = useState("");
  const joinSpace = useSpaceStore((s) => s.joinSpace);

  const buildConfigJson = useCallback((info: ShareInfo): string => {
    const config: Record<string, unknown> = {
      network_name: info.network_name,
      network_secret: info.network_secret,
    };
    if (info.virtual_ip) config.virtual_ipv4 = info.virtual_ip;
    if (info.dhcp !== undefined) config.dhcp = info.dhcp;
    if (info.peer_urls && info.peer_urls.length > 0) config.peer_urls = info.peer_urls;
    if (info.listener_urls && info.listener_urls.length > 0) {
      config.listener_urls = info.listener_urls;
    }
    return JSON.stringify(config);
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!networkName.trim() || !networkSecret.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await joinSpace(
        JSON.stringify({
          network_name: networkName.trim(),
          network_secret: networkSecret.trim(),
        })
      );
      onClose();
    } catch (e) {
      setError(String(e));
      toastError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleConfirmShare = async () => {
    if (!pendingShare) return;
    setLoading(true);
    setError(null);
    try {
      await joinSpace(buildConfigJson(pendingShare), pendingShare.name);
      onClose();
    } catch (e) {
      setError(String(e));
      toastError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handlePasteLink = async () => {
    try {
      const text = await readText();
      if (!text.trim()) return;
      const info = await resolveJoinShareInfo(text.trim());
      setPendingShare(info);
    } catch (e) {
      toastError(String(e));
    }
  };

  // 粘贴/手填分享链接（兜底入口，相机不可用时必经）
  const handleUseLink = useCallback(async () => {
    const text = pastedLink.trim();
    if (!text) return;
    try {
      const info = await resolveJoinShareInfo(text);
      setPendingShare(info);
    } catch (e) {
      toastError(String(e));
    }
  }, [pastedLink]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-96 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">
            {pendingShare ? t("space.confirmJoinTitle") : t("space.joinSpace")}
          </h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>

        {pendingShare ? (
          <div className="space-y-4">
            <div className="rounded-lg bg-[var(--color-surface-hover)] p-4 space-y-3">
              <div>
                <div className="text-xs text-[var(--color-text-secondary)]">
                  {t("settings.networkName")}
                </div>
                <div className="text-sm font-medium break-all">
                  {pendingShare.name || pendingShare.network_name}
                </div>
              </div>
              <div>
                <div className="text-xs text-[var(--color-text-secondary)]">
                  {t("space.confirmJoinIp")}
                </div>
                <div className="text-sm font-medium break-all">
                  {pendingShare.virtual_ip || t("space.dhcpAuto")}
                </div>
              </div>
            </div>
            {error && (
              <p className="text-xs text-[var(--color-danger)]">{error}</p>
            )}
            <Flex justify="end" gap="2" pt="2">
              <Button
                type="button"
                onClick={() => setPendingShare(null)}
                variant="outline"
                size="2"
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                disabled={loading}
                onClick={handleConfirmShare}
                variant="solid"
                color="blue"
                size="2"
                loading={loading}
              >
                {loading ? t("space.joining") : t("space.confirmJoin")}
              </Button>
            </Flex>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block mb-1 text-sm font-medium">
                {t("settings.networkName")}
              </label>
              <TextField.Root
                value={networkName}
                onChange={(e) => setNetworkName(e.target.value)}
                placeholder={t("space.spaceNamePlaceholder")}
                autoFocus
              />
            </div>
            <div>
              <label className="block mb-1 text-sm font-medium">
                {t("settings.networkSecret")}
              </label>
              <TextField.Root
                type="password"
                value={networkSecret}
                onChange={(e) => setNetworkSecret(e.target.value)}
                placeholder={t("space.networkSecretPlaceholder")}
              />
            </div>
            <div>
              <label className="block mb-1 text-sm font-medium">
                {t("space.pasteLinkLabel")}
              </label>
              <div className="flex items-center gap-2">
                <TextField.Root
                  value={pastedLink}
                  onChange={(e) => setPastedLink(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleUseLink();
                    }
                  }}
                  placeholder={t("space.pasteLinkPlaceholder")}
                />
                <Button
                  type="button"
                  onClick={handleUseLink}
                  variant="ghost"
                  color="blue"
                  size="2"
                  disabled={!pastedLink.trim()}
                >
                  {t("common.confirm")}
                </Button>
              </div>
              <div className="mt-2 flex items-center gap-2">
                <Button
                  type="button"
                  onClick={handlePasteLink}
                  variant="ghost"
                  color="blue"
                  size="1"
                >
                  {t("space.pasteShareLink")}
                </Button>
              </div>
            </div>
            {error && (
              <p className="text-xs text-[var(--color-danger)]">{error}</p>
            )}
            <Flex justify="end" gap="2" pt="2">
              <Button type="button" onClick={onClose} variant="outline" size="2">
                {t("common.cancel")}
              </Button>
              <Button
                type="submit"
                disabled={loading || !networkName.trim() || !networkSecret.trim()}
                variant="solid"
                color="blue"
                size="2"
                loading={loading}
              >
                {loading ? t("space.joining") : t("space.joinSpace")}
              </Button>
            </Flex>
          </form>
        )}
      </div>
    </div>
  );
}
