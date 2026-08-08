// src/utils/api.ts - API 入口，模块加载时按运行环境选定一套实现
// 模式在进入本文件时即已确定（apiMode），后续调用不再做任何环境判断。
import * as tauri from "./api/tauri";
import * as web from "./api/web";
import type { SendFileResult } from "./api/core";

export type ApiMode = "tauri" | "web";

/**
 * 当前运行模式：
 * - "tauri"：桌面（Tauri），走 invoke
 * - "web"  ：服务器（浏览器），走 fetch REST
 */
export const apiMode: ApiMode = isTauriEnv() ? "tauri" : "web";
export const isTauri = () => apiMode === "tauri";

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// 选定当前模式对应的实现（仅在此判断一次）
const impl = apiMode === "tauri" ? tauri : web;

// 以下全部为显式转发，类型与两套实现保持一致（均由 tauri 版签名定型）
export type {
  Space,
  Member,
  Message,
  FileInfo,
  NetworkStats,
  LogEntry,
  LogFilter,
  LogLevel,
  LogCategory,
  SpaceApp,
  PeerInfo,
  AclRule,
  PortForwardRule,
  ShareInfo,
} from "../types";
export type { NetworkConfig } from "../types/network";
export type {
  TraySpace,
  TrayLabels,
  SendFileResult,
  FileTransferProgress,
} from "./api/core";

export const createSpace = impl.createSpace;
export const joinSpace = impl.joinSpace;
export const leaveSpace = impl.leaveSpace;
export const deleteSpace = impl.deleteSpace;
export const listSpaces = impl.listSpaces;
export const generateShareLink = impl.generateShareLink;
export const parseShareLink = impl.parseShareLink;
export const connectSpace = impl.connectSpace;
export const disconnectSpace = impl.disconnectSpace;
export const listMembers = impl.listMembers;
export const syncTrayMenu = impl.syncTrayMenu;

export const getNetworkStats = impl.getNetworkStats;

export const sendMessage = impl.sendMessage;
export const getMessageHistory = impl.getMessageHistory;
export const sendSignal = impl.sendSignal;

export const updateSpaceConfig = impl.updateSpaceConfig;

export const receiveFile = impl.receiveFile as (
  spaceId: string,
  fileId: string,
  savePath?: string,
  password?: string
) => Promise<void>;
export const sendFile = impl.sendFile as (
  spaceId: string,
  file: string | File,
  password?: string
) => Promise<SendFileResult>;
export const recordReceivedFile = impl.recordReceivedFile;
export const deleteFile = impl.deleteFile;
export const listFiles = impl.listFiles;
export const getTransferProgress = impl.getTransferProgress;

export const getLogs = impl.getLogs;
export const getSpaceLogs = impl.getSpaceLogs;
export const clearLogs = impl.clearLogs;
export const queryLogs = impl.queryLogs;
export const getLogModules = impl.getLogModules;
export const clearLogsFiltered = impl.clearLogsFiltered;
export const exportLogs = impl.exportLogs;
export const getLogEnabled = impl.getLogEnabled;
export const setLogEnabled = impl.setLogEnabled;

export const getSystemConfig = impl.getSystemConfig;
export const setSystemConfig = impl.setSystemConfig;

export const getAppConfig = impl.getAppConfig;
export const setAppConfig = impl.setAppConfig;
export const getConfigFilePath = impl.getConfigFilePath;
export const getConfigTemplatePath = impl.getConfigTemplatePath;

export const getSpacePeers = impl.getSpacePeers;

export const addApp = impl.addApp;
export const updateApp = impl.updateApp;
export const deleteApp = impl.deleteApp;
export const listApps = impl.listApps;
export const shareApp = impl.shareApp;

export const isDaemonReady = impl.isDaemonReady;
export const getDaemonErrorReason = impl.getDaemonErrorReason;

export const getEasyTierVersion = impl.getEasyTierVersion;
export const checkEasyTierUpdate = impl.checkEasyTierUpdate;
export const upgradeEasyTierWithProgress = impl.upgradeEasyTierWithProgress;

export const getAclRules = impl.getAclRules;
export const createAclRule = impl.createAclRule;
export const updateAclRule = impl.updateAclRule;
export const deleteAclRule = impl.deleteAclRule;

export const getPortForwardRules = impl.getPortForwardRules;
export const createPortForwardRule = impl.createPortForwardRule;
export const updatePortForwardRule = impl.updatePortForwardRule;
export const deletePortForwardRule = impl.deletePortForwardRule;