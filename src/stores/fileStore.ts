import { create } from "zustand";
import type { FileInfo, TransferProgress } from "../types";

interface FileStore {
  files: Record<string, FileInfo[]>;
  transfers: Record<string, TransferProgress>;
  loading: boolean;

  setFiles: (spaceId: string, files: FileInfo[]) => void;
  addFile: (spaceId: string, file: FileInfo) => void;
  updateTransfer: (transferId: string, progress: TransferProgress) => void;
  removeTransfer: (transferId: string) => void;
}

export const useFileStore = create<FileStore>((set) => ({
  files: {},
  transfers: {},
  loading: false,

  setFiles: (spaceId, files) =>
    set((state) => ({ files: { ...state.files, [spaceId]: files } })),

  addFile: (spaceId, file) =>
    set((state) => {
      const existing = state.files[spaceId] || [];
      return { files: { ...state.files, [spaceId]: [...existing, file] } };
    }),

  updateTransfer: (transferId, progress) =>
    set((state) => ({
      transfers: { ...state.transfers, [transferId]: progress },
    })),

  removeTransfer: (transferId) =>
    set((state) => {
      const { [transferId]: _, ...rest } = state.transfers;
      return { transfers: rest };
    }),
}));