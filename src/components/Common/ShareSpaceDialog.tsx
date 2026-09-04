import { QRCodeSVG } from "qrcode.react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, TextField, Flex } from "@radix-ui/themes";
import Tip from "../Common/Tip";
import { X, Copy, Check, HelpCircle } from "lucide-react";
import { toastSuccess, toastError } from "../../utils/toast";
import { generateShareLink } from "../../utils/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

interface ShareSpaceDialogProps {
  spaceId: string;
  onClose: () => void;
}

export function ShareSpaceDialog({ spaceId, onClose }: ShareSpaceDialogProps) {
  const { t } = useTranslation();
  const [ip, setIp] = useState<string>("");
  const [link, setLink] = useState<string>("");
  const [copied, setCopied] = useState(false);
  const [generating, setGenerating] = useState(false);

  const handleCreate = async () => {
    setGenerating(true);
    try {
      const newLink = await generateShareLink(spaceId, ip.trim() || undefined);
      setLink(newLink);
    } catch (err) {
      console.log(err);
      toastError(t("space.shareFailed"));
    } finally {
      setGenerating(false);
    }
  };

  const handleCopy = async () => {
    try {
      await writeText(link);
      setCopied(true);
      toastSuccess(t("space.copiedToClipboard"));
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.log(err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-full max-w-[calc(100vw-24px)] sm:w-[340px] shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t("space.shareSpace")}</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>

        <div className="space-y-2">
          <Flex align="center">
            <label className="block text-sm font-medium">
              {t("space.setReceiverIp")}
            </label>
            <Tip
              content={
                <>
                  <p className="mt-1 text-xs">
                    {t("space.joinWithShareConfig")}
                  </p>
                  <p className="mt-1 text-xs">
                    {t("space.dhcpHint")}
                  </p>
                </>
              }
            >
              <span className="inline-flex items-center cursor-pointer text-[var(--color-text-secondary)]">
                <HelpCircle size={14} />
              </span>
            </Tip>
          </Flex>
            <TextField.Root
              value={ip}
              onChange={(e) => setIp(e.target.value)}
              placeholder={t("space.receiverIpPlaceholder")}
            />

          {link && (
            <>
              <div className="flex justify-center">
                <div className="p-3 bg-white rounded-xl w-full max-w-[284px]">
                  <div className="w-full aspect-square">
                    <QRCodeSVG value={link} size={260} className="w-full h-full" />
                  </div>
                </div>
              </div>

              <Flex align="center" gap="3">
                <TextField.Root
                  value={link}
                  readOnly
                  className="flex-1 font-mono text-xs"
                />
                <Button onClick={handleCopy} variant="ghost" size="2">
                  {copied ? <Check size={18} /> : <Copy size={18} />}
                </Button>
              </Flex>
            </>
          )}
          <Flex className={`pt-${link ? '0' : '2'}`}>
            <Button
              onClick={handleCreate}
              disabled={generating}
              className="w-full mt-2"
              size="2"
            >
              {generating ? t("common.loading") : t("space.createShare")}
            </Button>
          </Flex>
        </div>
      </div>
    </div>
  );
}
