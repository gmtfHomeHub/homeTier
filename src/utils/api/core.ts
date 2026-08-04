// src/utils/api/core.ts - 核心类型导出
export type {
  Space,
  Member,
  Message,
  FileInfo,
  NetworkStats,
  LogEntry,
  SpaceApp,
  PeerInfo,
  AclRule,
  PortForwardRule,
  ShareInfo,
} from "../../types";

export type { NetworkConfig } from "../../types/network";

export interface TraySpace {
  id: string;
  name: string;
}

export interface TrayLabels {
  show: string;
  quit: string;
}

export interface SendFileResult {
  transfer_id: string;
  file_info: import("../../types").FileInfo;
}

export interface FileTransferProgress {
  transfer_id: string;
  file_name: string;
  bytes_transferred: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  status: "Transferring" | "Paused" | "Completed" | "Failed";
}