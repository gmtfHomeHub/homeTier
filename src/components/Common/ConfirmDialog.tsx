import { useState } from "react";
import { X, AlertTriangle } from "lucide-react";
import { Dialog, Button, Flex } from "@radix-ui/themes";
import { toastError } from "../../utils/toast";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  message,
  confirmText = "确定",
  cancelText = "取消",
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const [loading, setLoading] = useState(false);

  const handleConfirm = async () => {
    setLoading(true);
    try {
      await onConfirm();
    } catch (e) {
      toastError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={(open) => { if (!open) onCancel(); }}>
      <Dialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-[var(--color-surface)] rounded-xl p-6 w-80 shadow-xl animate-fade-in z-50 focus:outline-none">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            {danger && <AlertTriangle size={20} className="text-[var(--color-danger)]" />}
            <Dialog.Title className="text-lg font-semibold m-0">{title}</Dialog.Title>
          </div>
          <Dialog.Close className="p-1 rounded hover:bg-[var(--color-border)]">
            <X size={20} />
          </Dialog.Close>
        </div>
        <Dialog.Description className="text-sm text-[var(--color-text-secondary)] mb-6">
          {message}
        </Dialog.Description>
        <Flex justify="end" gap="2">
          <Dialog.Close>
            <Button variant="outline" size="2" disabled={loading}>
              {cancelText}
            </Button>
          </Dialog.Close>
          <Button
            onClick={handleConfirm}
            disabled={loading}
            variant={danger ? "solid" : "solid"}
            color={danger ? "red" : "blue"}
            size="2"
            loading={loading}
          >
            {loading ? "处理中..." : confirmText}
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}