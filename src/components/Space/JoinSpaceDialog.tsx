import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { parseShareLink } from "../../utils/api";
import { detectDeviceMode } from "../../utils/device";
import type { ShareInfo } from "../../types";
import { X, QrCode } from "lucide-react";
import { Button, TextField, Flex } from "@radix-ui/themes";
import { toastError, toastInfo } from "../../utils/toast";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { scan, cancel as cancelScan, requestPermissions, Format } from "@tauri-apps/plugin-barcode-scanner";

interface JoinSpaceDialogProps {
  onClose: () => void;
}

export function JoinSpaceDialog({ onClose }: JoinSpaceDialogProps) {
  const { t } = useTranslation();
  const [networkName, setNetworkName] = useState("");
  const [networkSecret, setNetworkSecret] = useState("");
  const [pendingShare, setPendingShare] = useState<ShareInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const joinSpace = useSpaceStore((s) => s.joinSpace);

  const isMobile = detectDeviceMode() === "mobile";

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
      const info = await parseShareLink(text.trim());
      setPendingShare(info);
    } catch (e) {
      toastError(String(e));
    }
  };

  // 扫描期间拦截 Android 硬件返回键 → 取消扫描而非关闭 App
  useEffect(() => {
    if (!scanning) return;
    const onPopState = () => cancelScan().catch(() => {});
    // push 一个 dummy state，使 WebView canGoBack()=true，返回键触发 popstate 而非 finish activity
    window.history.pushState({ qrScanning: true }, "");
    window.addEventListener("popstate", onPopState);
    return () => {
      window.removeEventListener("popstate", onPopState);
      if (window.history.state?.qrScanning) window.history.back();
    };
  }, [scanning]);

  const startScan = useCallback(async () => {
    setScanning(true);
    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    try {
      // 请求相机权限
      const perm = await requestPermissions();
      if (perm !== "granted") {
        toastError(t("space.cameraUnavailable"));
        return;
      }
      // 提示用户对准二维码
      toastInfo(t("space.scanningQR"));
      // 超时自动取消（60秒无识别则退出扫描）
      timeoutId = setTimeout(() => {
        cancelScan().catch(() => {});
      }, 60000);
      // 原生全屏扫描，指定 QR 格式 + 后置摄像头
      const result = await scan({
        formats: [Format.QRCode],
        cameraDirection: "back",
      });
      // 扫描成功，解析分享链接
      const info = await parseShareLink(result.content);
      setPendingShare(info);
    } catch (e) {
      // 用户取消（含超时取消、返回键取消）时 scan() reject "cancelled"，不报错
      const msg = String(e);
      if (!msg.toLowerCase().includes("cancel")) {
        toastError(msg);
      }
    } finally {
      if (timeoutId) clearTimeout(timeoutId);
      setScanning(false);
    }
  }, [t]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-96 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">
            {pendingShare ? t("space.confirmJoinTitle") : t("space.joinSpace")}
          </h2>
          <div className="flex items-center gap-1">
            {isMobile && !scanning && !pendingShare && (
              <Button onClick={startScan} variant="ghost" size="2">
                <QrCode size={20} />
              </Button>
            )}
            <Button onClick={onClose} variant="ghost" size="2">
              <X size={20} />
            </Button>
          </div>
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
            <div className="flex items-center gap-2">
              <Button type="button" onClick={handlePasteLink} variant="ghost" color="blue" size="1">
                {t("space.pasteShareLink")}
              </Button>
              {isMobile && (
                <span className="text-xs text-[var(--color-text-secondary)]">
                  {t("space.scanToJoin")}
                </span>
              )}
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
