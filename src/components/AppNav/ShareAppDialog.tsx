import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Share2, X } from "lucide-react";
import { Button, Dialog, Flex, Text, Box } from "@radix-ui/themes";
import * as api from "../../utils/api";
import type { Space, SpaceApp } from "../../types";
import { toastError } from "../../utils/toast";

interface ShareAppDialogProps {
  app: SpaceApp;
  currentSpaceId: string;
  onClose: () => void;
  onShared: () => void;
}

export function ShareAppDialog({
  app,
  currentSpaceId,
  onClose,
  onShared,
}: ShareAppDialogProps) {
  const { t } = useTranslation();
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [sharingId, setSharingId] = useState<string | null>(null);

  useEffect(() => {
    api
      .listSpaces()
      .then((list) => setSpaces(list.filter((s) => s.id !== currentSpaceId)))
      .catch((e) => toastError(String(e)));
  }, [currentSpaceId]);

  const handleShare = async (targetSpaceId: string) => {
    setSharingId(targetSpaceId);
    try {
      await api.shareApp(app.id, targetSpaceId);
      onShared();
      onClose();
    } catch (e) {
      toastError(String(e));
    } finally {
      setSharingId(null);
    }
  };

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Content maxWidth="420px" className="w-full max-w-[calc(100vw-24px)] sm:w-[420px]">
        <Flex justify="between" align="center" mb="3">
          <Text size="4" weight="bold">
            {t("appNav.shareApp")}「{app.name}」
          </Text>
          <Button variant="ghost" size="1" onClick={onClose}>
            <X size={14} />
          </Button>
        </Flex>
        <Text size="2" color="gray" mb="3" className="block">
          {t("appNav.shareDesc")}
        </Text>
        {spaces.length === 0 ? (
          <Text size="2" color="gray">
            {t("appNav.noSpacesToShare")}
          </Text>
        ) : (
          <Box className="space-y-2 max-h-[300px] overflow-y-auto">
            {spaces.map((s) => (
              <Flex
                key={s.id}
                justify="between"
                align="center"
                p="2"
                className="rounded bg-[var(--color-panel)] border border-[var(--color-border)]"
              >
                <Box>
                  <Text as="div" size="2" weight="bold">
                    {s.name}
                  </Text>
                  {s.description && (
                    <Text as="div" size="1" color="gray">
                      {s.description}
                    </Text>
                  )}
                </Box>
                <Button
                  size="1"
                  variant="soft"
                  disabled={sharingId === s.id}
                  onClick={() => handleShare(s.id)}
                >
                  <Share2 size={12} /> {sharingId === s.id ? t("appNav.sharing") : t("appNav.shareApp", { name: "" }).replace(/「.*」/, "")}
                </Button>
              </Flex>
            ))}
          </Box>
        )}
      </Dialog.Content>
    </Dialog.Root>
  );
}
