import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { X, Eye, EyeOff } from "lucide-react";
import { Button, TextField, TextArea, Flex } from "@radix-ui/themes";
import { toastError } from "../../utils/toast";

interface CreateSpaceDialogProps {
  onClose: () => void;
}

export function CreateSpaceDialog({ onClose }: CreateSpaceDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [networkSecret, setNetworkSecret] = useState("");
  const [description, setDescription] = useState("");
  const [loading, setLoading] = useState(false);
  const [showSecret, setShowSecret] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const createSpace = useSpaceStore((s) => s.createSpace);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !networkSecret.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const ownerId = crypto.randomUUID();
      await createSpace(name.trim(), networkSecret.trim(), ownerId, description.trim() || undefined);
      onClose();
    } catch (e) {
      setError(String(e));
      toastError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-96 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t("space.createSpace")}</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1">{t("space.spaceName")}</label>
            <TextField.Root
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("space.spaceNamePlaceholder")}
              autoFocus
            />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1">{t("settings.networkSecret")}</label>
            <TextField.Root
              type={showSecret ? "text" : "password"}
              value={networkSecret}
              onChange={(e) => setNetworkSecret(e.target.value)}
              placeholder={t("space.networkSecretPlaceholder")}
            >
              <TextField.Slot side="right">
                <Button
                  type="button"
                  onClick={() => setShowSecret(!showSecret)}
                  variant="ghost"
                  size="1"
                >
                  {showSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                </Button>
              </TextField.Slot>
            </TextField.Root>
          </div>
          <div>
            <label className="block text-sm font-medium mb-1">{t("space.descriptionOptional")}</label>
            <TextArea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("space.descriptionPlaceholder")}
              rows={3}
            />
          </div>
          {error && (
            <p className="text-xs text-[var(--color-danger)]">{error}</p>
          )}
          <Flex justify="end" gap="2" pt="2">
            <Button type="button" onClick={onClose} variant="outline" size="2">
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={loading || !name.trim() || !networkSecret.trim()} variant="solid" color="blue" size="2" loading={loading}>
              {loading ? t("space.creating") : t("space.createSpace")}
            </Button>
          </Flex>
        </form>
      </div>
    </div>
  );
}
