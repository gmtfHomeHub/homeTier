import { QRCodeSVG } from "qrcode.react";
import { useState } from "react";
import { Button, TextField } from "@radix-ui/themes";
import { X, Copy, Check } from "lucide-react";
import { generateShareLink } from "../../utils/api";

interface ShareSpaceDialogProps {
  spaceId: string;
  onClose: () => void;
}

export function ShareSpaceDialog({ spaceId, onClose }: ShareSpaceDialogProps) {
  const [link, setLink] = useState<string>("");
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);

  useState(() => {
    generateShareLink(spaceId)
      .then(setLink)
      .finally(() => setLoading(false));
  });

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {}
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-80 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">分享空间</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>

        {loading ? (
          <div className="text-center py-8 text-[var(--color-text-secondary)]">加载中...</div>
        ) : (
          <div className="space-y-4">
            {/* 二维码 */}
            <div className="flex justify-center">
              <div className="bg-white p-3 rounded-xl">
                <QRCodeSVG value={link} size={180} />
              </div>
            </div>

            {/* 链接 */}
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
              <p className="text-xs text-[var(--color-success)] text-center">已复制到剪贴板</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}