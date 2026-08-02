import { QRCodeSVG } from "qrcode.react";
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Button, TextField } from "@radix-ui/themes";
import { X, Copy, Check } from "lucide-react";
import { generateShareLink } from "../../utils/api";

interface ShareSpaceDialogProps {
  spaceId: string;
  onClose: () => void;
}

export function ShareSpaceDialog({ spaceId, onClose }: ShareSpaceDialogProps) {
  const { t } = useTranslation();
  const [link, setLink] = useState<string>("");
  const [ip, setIp] = useState<string>("");
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    const timer = setTimeout(() => {
      generateShareLink(spaceId, ip.trim() || undefined)
        .then(setLink)
        .catch(() => setLink(""))
        .finally(() => setLoading(false));
    }, 300);
    return () => clearTimeout(timer);
  }, [spaceId, ip]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.log(err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-80 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t("space.shareSpace")}</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>

        {loading ? (
          <div className="text-center py-8 text-[var(--color-text-secondary)]">{t("common.loading")}</div>
        ) : (
          <div className="space-y-4">
            <div>
              <label className="block mb-1 text-sm font-medium">
                {t("space.setReceiverIp")}
              </label>
              <TextField.Root
                value={ip}
                onChange={(e) => setIp(e.target.value)}
                placeholder={t("space.receiverIpPlaceholder")}
              />
              <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
                {t("space.joinWithShareConfig")}
              </p>
            </div>

            <div className="flex justify-center">
              <div className="p-3 bg-white rounded-xl">
                <QRCodeSVG value={link} size={180} />
              </div>
            </div>

            <div className="flex items-center gap-2">
              <TextField.Root
                value={link}
                readOnly
                className="flex-1 font-mono text-xs"
              />
              <Button
                onClick={handleCopy}
                variant="ghost"
                size="2"
              >
                {copied ? <Check size={18} /> : <Copy size={18} />}
              </Button>
            </div>
            {copied && (
              <p className="text-xs text-[var(--color-success)] text-center">{t("space.copiedToClipboard")}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
