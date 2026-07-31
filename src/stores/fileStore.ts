import { create } from "zustand";
import type { FileInfo } from "../types";

interface FileStore {
  files: Record<string, FileInfo[]>;

  setFiles: (spaceId: string, files: FileInfo[]) => void;
}

export const useFileStore = create<FileStore>((set) => ({
  files: {},

  setFiles: (spaceId, files) =>
    set((state) => ({ files: { ...state.files, [spaceId]: files } })),
}));