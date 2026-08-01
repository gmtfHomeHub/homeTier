import { create } from "zustand";
import type { FileInfo } from "../types";
import type { FileTransferProgress } from "../utils/api";

export interface FileTransferItem {
  id: string;
  fileId?: string;
  file_name: string;
  bytes_transferred: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  status: FileTransferProgress["status"];
  /** true = 上传（发送） */
  isUpload: boolean;
  error?: string;
}

interface FileStore {
  files: Record<string, FileInfo[]>;
  /** 活跃传输：key 为 transfer_id（上传）或 file_id（下载） */
  transfers: Record<string, FileTransferItem>;
  /** 批量发送队列 */
  sendQueue: Record<string, FileTransferItem[]>;

  setFiles: (spaceId: string, files: FileInfo[]) => void;
  addFile: (spaceId: string, file: FileInfo) => void;
  removeFile: (spaceId: string, fileId: string) => void;
  upsertTransfer: (id: string, item: Partial<FileTransferItem> & Pick<FileTransferItem, "file_name" | "isUpload">) => void;
  removeTransfer: (id: string) => void;
  setSendQueue: (spaceId: string, items: FileTransferItem[]) => void;
  updateSendQueueItem: (spaceId: string, id: string, patch: Partial<FileTransferItem>) => void;
  clearTransfers: () => void;
}

export const useFileStore = create<FileStore>((set) => ({
  files: {},
  transfers: {},
  sendQueue: {},

  setFiles: (spaceId, files) =>
    set((state) => ({ files: { ...state.files, [spaceId]: files } })),

  addFile: (spaceId, file) =>
    set((state) => ({
      files: {
        ...state.files,
        [spaceId]: [file, ...(state.files[spaceId] || [])],
      },
    })),

  removeFile: (spaceId, fileId) =>
    set((state) => ({
      files: {
        ...state.files,
        [spaceId]: (state.files[spaceId] || []).filter((f) => f.id !== fileId),
      },
    })),

  upsertTransfer: (id, item) =>
    set((state) => ({
      transfers: {
        ...state.transfers,
        [id]: { ...(state.transfers[id] as FileTransferItem | undefined), ...item } as FileTransferItem,
      },
    })),

  removeTransfer: (id) =>
    set((state) => {
      const transfers = { ...state.transfers };
      delete transfers[id];
      return { transfers };
    }),

  setSendQueue: (spaceId, items) =>
    set((state) => ({ sendQueue: { ...state.sendQueue, [spaceId]: items } })),

  updateSendQueueItem: (spaceId, id, patch) =>
    set((state) => ({
      sendQueue: {
        ...state.sendQueue,
        [spaceId]: (state.sendQueue[spaceId] || []).map((it) =>
          it.id === id ? { ...it, ...patch } : it
        ),
      },
    })),

  clearTransfers: () => set({ transfers: {}, sendQueue: {} }),
}));
