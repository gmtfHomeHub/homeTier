import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { useFileStore } from "../../stores/fileStore";
import type { FileTransferItem } from "../../stores/fileStore";
import { formatFileSize, formatTimestamp } from "../../utils/format";
import * as api from "../../utils/api";
import type { FileInfo } from "../../types";
import { Button, Dialog, TextField, Progress, Text, Flex, AlertDialog } from "@radix-ui/themes";
import { Download, Lock, FileText, ArrowLeft, Upload, Trash2, CheckCircle2, Loader2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { open, save } from "@tauri-apps/plugin-dialog";
import { isTauri } from "../../utils/api";
import { sendSignal } from "../../services/signal";
import { v4 as uuidv4 } from "uuid";

export function FileList() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { files, setFiles, addFile, removeFile, sendQueue, setSendQueue, updateSendQueueItem } = useFileStore();
  const [loading, setLoading] = useState(false);
  const { t } = useTranslation();

  // Web 模式：隐藏文件选择器
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 密码 Dialog 状态
  const [passwordFile, setPasswordFile] = useState<FileInfo | null>(null);
  const [passwordInput, setPasswordInput] = useState("");
  const [passwordError, setPasswordError] = useState("");
  // 下载进度：key = file_id
  const [downloadStates, setDownloadStates] = useState<Record<string, "downloading" | "done" | "failed">>({});
  // 删除确认
  const [deleteFileInfo, setDeleteFileInfo] = useState<FileInfo | null>(null);

  const spaceFiles = id ? files[id] || [] : [];
  const queue = id ? sendQueue[id] || [] : [];

  const busyRef = useRef(false);

  useEffect(() => {
    if (id) {
      loadFiles();
    }
  }, [id]);

  const loadFiles = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const result = await api.listFiles(id);
      setFiles(id, result);
    } catch (e) {
      console.error("Load files failed:", e);
    } finally {
      setLoading(false);
    }
  };

  /** 批量选择文件并发送（Tauri 用系统对话框，Web 用隐藏 input） */
  const handleFileSelect = async () => {
    if (!id) return;
    if (!isTauri()) {
      fileInputRef.current?.click();
      return;
    }
    const selected = await open({ multiple: true });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;

    const items = paths.map((p) => ({
      id: uuidv4(),
      file_name: p.split(/[\\/]/).pop() || p,
      bytes_transferred: 0,
      total_bytes: 0,
      speed_bytes_per_sec: 0,
      status: "Transferring" as const,
      isUpload: true,
    }));
    setSendQueue(id, [...queue, ...items]);
    void sendQueueSequentially(id, paths, items);
  };

  /** Web 模式：input 选择文件后发送 */
  const handleFileInputChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!id) return;
    const files = Array.from(e.target.files || []);
    e.target.value = "";
    if (files.length === 0) return;

    const items = files.map((f) => ({
      id: uuidv4(),
      file_name: f.name,
      bytes_transferred: 0,
      total_bytes: 0,
      speed_bytes_per_sec: 0,
      status: "Transferring" as const,
      isUpload: true,
    }));
    setSendQueue(id, [...queue, ...items]);
    void sendQueueSequentially(id, files, items);
  };

  /** 依次发送队列中的文件 */
  const sendQueueSequentially = async (
    spaceId: string,
    paths: (string | File)[],
    items: FileTransferItem[],
    password?: string
  ) => {
    if (busyRef.current) return;
    busyRef.current = true;
    try {
      for (let i = 0; i < paths.length; i++) {
        updateSendQueueItem(spaceId, items[i].id, { status: "Transferring", total_bytes: 0 });
        try {
          const result = await api.sendFile(spaceId, paths[i], password);
          updateSendQueueItem(spaceId, items[i].id, {
            status: "Completed",
            fileId: result.file_info.id,
            bytes_transferred: result.file_info.file_size,
            total_bytes: result.file_info.file_size,
          });
          addFile(spaceId, result.file_info);
          // 广播文件已发送（接收端刷新 + 通知）
          void sendSignal(spaceId, "file", "sent", { file: result.file_info });
        } catch (err) {
          console.error("Send file failed:", err);
          updateSendQueueItem(spaceId, items[i].id, { status: "Failed" });
        }
      }
    } finally {
      busyRef.current = false;
      // 队列完成后清除已完成项
      const remaining = useFileStore
        .getState()
        .sendQueue[spaceId]?.filter((it) => it.status === "Transferring" || it.status === "Failed") || [];
      setSendQueue(spaceId, remaining);
    }
  };

  const handleDownload = async (file: FileInfo) => {
    if (!id) return;
    if (file.is_password_protected) {
      setPasswordFile(file);
      setPasswordInput("");
      setPasswordError("");
      return;
    }
    void doDownload(id, file);
  };

  const doDownload = async (spaceId: string, file: FileInfo, password?: string) => {
    setDownloadStates((s) => ({ ...s, [file.id]: "downloading" }));
    try {
      if (!isTauri()) {
        await api.receiveFile(spaceId, file.id, undefined, password);
        setDownloadStates((s) => ({ ...s, [file.id]: "done" }));
        return;
      }
      const savePath = await save({
        defaultPath: file.file_name,
      });
      if (!savePath) {
        setDownloadStates((s) => ({ ...s, [file.id]: "done" }));
        return;
      }
      await api.receiveFile(spaceId, file.id, savePath, password);
      setDownloadStates((s) => ({ ...s, [file.id]: "done" }));
    } catch (err) {
      console.error("Download file failed:", err);
      if (file.is_password_protected && password) {
        setPasswordError(t("file.enterPassword"));
      }
      setDownloadStates((s) => ({ ...s, [file.id]: "failed" }));
    }
  };

  const confirmPasswordDownload = async () => {
    if (!passwordFile || !id) return;
    if (!passwordInput.trim()) {
      setPasswordError(t("file.enterPassword"));
      return;
    }
    setPasswordFile(null);
    void doDownload(id, passwordFile, passwordInput.trim());
  };

  const handleDelete = async (file: FileInfo) => {
    if (!id) return;
    try {
      await api.deleteFile(id, file.id);
      removeFile(id, file.id);
    } catch (e) {
      console.error("Delete file failed:", e);
    } finally {
      setDeleteFileInfo(null);
    }
  };

  const queueDone = queue.filter((q) => q.status === "Completed").length;

  return (
    <div className="flex flex-col flex-1">
      <div className="h-14 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)]">
        <Button
          onClick={() => navigate(`/space/${id}`)}
          variant="ghost"
          size="2"
        >
          <ArrowLeft size={20} />
        </Button>
        <span className="font-semibold">{t('file.title')}</span>
        <div className="flex-1" />
        <input
          ref={fileInputRef}
          type="file"
          multiple
          className="hidden"
          onChange={handleFileInputChange}
        />
        <Button onClick={handleFileSelect} variant="solid" color="blue" size="2">
          <Upload size={16} />
          {t('file.batchSend')}
        </Button>
      </div>

      <div className="flex-1 p-4 overflow-y-auto">
        {loading ? (
          <div className="text-center py-8 text-[var(--color-text-secondary)]">{t('common.loading')}</div>
        ) : (
          <>
            {queue.length > 0 && (
              <div className="mb-4 space-y-2">
                {queue.map((q) => (
                  <div
                    key={q.id}
                    className="flex items-center gap-3 p-3 rounded-xl bg-[var(--color-surface)] border border-[var(--color-border)]"
                  >
                    {q.status === "Completed" ? (
                      <CheckCircle2 size={20} className="text-green-500 shrink-0" />
                    ) : q.status === "Failed" ? (
                      <Loader2 size={20} className="text-red-500 shrink-0" />
                    ) : (
                      <Loader2 size={20} className="animate-spin text-[var(--color-primary)] shrink-0" />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{q.file_name}</div>
                      <div className="text-xs text-[var(--color-text-secondary)]">
                        {q.status === "Completed"
                          ? t('file.completed')
                          : q.status === "Failed"
                            ? t('file.failed')
                            : t('file.uploading')}
                      </div>
                    </div>
                    {q.status === "Transferring" && (
                      <Progress value={undefined} size="1" className="w-24" />
                    )}
                  </div>
                ))}
                {queue.length > 1 && (
                  <div className="text-xs text-[var(--color-text-secondary)] px-1">
                    {t('file.progress')}: {queueDone}/{queue.length}
                  </div>
                )}
              </div>
            )}

            {spaceFiles.length === 0 && queue.length === 0 ? (
              <div className="text-center py-20 text-[var(--color-text-secondary)]">
                <FileText size={48} className="mx-auto mb-3 opacity-50" />
                <p>{t('file.noFiles')}</p>
              </div>
            ) : (
              <div className="space-y-2">
                {spaceFiles.map((file) => (
                  <div
                    key={file.id}
                    className="flex items-center gap-3 p-3 rounded-xl bg-[var(--color-surface)] border border-[var(--color-border)]"
                  >
                    <FileText size={24} className="text-[var(--color-primary)] shrink-0" />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{file.file_name}</div>
                      <div className="text-xs text-[var(--color-text-secondary)]">
                        {formatFileSize(file.file_size)}
                        {file.is_compressed && ` · ${t('file.compressed')}`}
                        {file.is_password_protected && (
                          <span className="ml-1 inline-flex items-center gap-0.5">
                            <Lock size={10} /> {t('file.encrypted')}
                          </span>
                        )}
                        <span className="ml-2">{formatTimestamp(file.created_at)}</span>
                      </div>
                      {downloadStates[file.id] === "done" && (
                        <div className="text-xs text-green-500 mt-0.5 flex items-center gap-1">
                          <CheckCircle2 size={12} /> {t('file.verified')}
                        </div>
                      )}
                      {downloadStates[file.id] === "failed" && (
                        <div className="text-xs text-red-500 mt-0.5">{t('file.failed')}</div>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="2"
                      title={t('file.download')}
                      disabled={downloadStates[file.id] === "downloading"}
                      onClick={() => handleDownload(file)}
                    >
                      {downloadStates[file.id] === "downloading" ? (
                        <Loader2 size={18} className="animate-spin" />
                      ) : (
                        <Download size={18} />
                      )}
                    </Button>
                    <Button
                      variant="ghost"
                      size="2"
                      color="red"
                      title={t('file.delete')}
                      onClick={() => setDeleteFileInfo(file)}
                    >
                      <Trash2 size={18} />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </div>

      {/* 密码 Dialog */}
      <Dialog.Root open={passwordFile !== null} onOpenChange={(open) => !open && setPasswordFile(null)}>
        <Dialog.Content className="w-full max-w-[calc(100vw-24px)] sm:w-[420px]">
          <Dialog.Title>{t('file.enterPassword')}</Dialog.Title>
          <Flex direction="column" gap="3" mt="3">
            <Text size="2" color="gray">
              {passwordFile?.file_name}
            </Text>
            <TextField.Root
              type="password"
              placeholder={t('file.password')}
              value={passwordInput}
              onChange={(e) => {
                setPasswordInput(e.target.value);
                setPasswordError("");
              }}
              onKeyDown={(e) => e.key === "Enter" && void confirmPasswordDownload()}
            />
            {passwordError && <Text size="1" color="red">{passwordError}</Text>}
            <Flex gap="3" justify="end">
              <Button variant="soft" onClick={() => setPasswordFile(null)}>{t('file.cancel')}</Button>
              <Button onClick={() => void confirmPasswordDownload()} color="blue">
                {t('file.confirm')}
              </Button>
            </Flex>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>

      {/* 删除确认 */}
      <AlertDialog.Root open={deleteFileInfo !== null} onOpenChange={(open) => !open && setDeleteFileInfo(null)}>
        <AlertDialog.Content className="w-full max-w-[calc(100vw-24px)] sm:w-[420px]">
          <AlertDialog.Title>{t('file.delete')}</AlertDialog.Title>
          <AlertDialog.Description size="2" color="gray">
            {deleteFileInfo?.file_name}
          </AlertDialog.Description>
          <Flex gap="3" mt="4" justify="end">
            <AlertDialog.Cancel>
              <Button variant="soft">{t('file.cancel')}</Button>
            </AlertDialog.Cancel>
            <AlertDialog.Action>
              <Button color="red" onClick={() => void handleDelete(deleteFileInfo!)}>
                {t('file.delete')}
              </Button>
            </AlertDialog.Action>
          </Flex>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </div>
  );
}
