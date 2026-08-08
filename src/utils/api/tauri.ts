// src/utils/api/tauri.ts - Tauri 桌面端实现
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
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
  TraySpace,
  TrayLabels,
  SendFileResult,
  FileTransferProgress,
} from "./core";

export async function createSpace(
  name: string,
  networkSecret: string,
  description?: string
): Promise<Space> {
  return invoke("create_space", { name, networkSecret, description });
}

export async function joinSpace(configJson: string): Promise<Space> {
  return invoke("join_space", { configJson });
}

export async function leaveSpace(spaceId: string): Promise<void> {
  return invoke("leave_space", { spaceId });
}

export async function deleteSpace(spaceId: string): Promise<void> {
  return invoke("delete_space", { spaceId });
}

export async function listSpaces(): Promise<Space[]> {
  return invoke("list_spaces");
}

export async function generateShareLink(spaceId: string, ip?: string): Promise<string> {
  return invoke("generate_share_link", { spaceId, ip });
}

export async function parseShareLink(link: string): Promise<ShareInfo> {
  return invoke("parse_share_link", { link });
}

export async function connectSpace(spaceId: string): Promise<void> {
  return invoke("connect_space", { spaceId });
}

export async function disconnectSpace(spaceId: string): Promise<void> {
  return invoke("disconnect_space", { spaceId });
}

export async function listMembers(spaceId: string): Promise<Member[]> {
  return invoke("list_members", { spaceId });
}

export async function syncTrayMenu(spaces: TraySpace[], labels: TrayLabels): Promise<void> {
  return invoke("update_tray_menu", { spaces, labels });
}

export async function getNetworkStats(spaceId: string): Promise<NetworkStats> {
  return invoke<NetworkStats>("get_network_stats", { spaceId });
}

export async function sendMessage(
  spaceId: string,
  content: string,
  msgType: string
): Promise<Message> {
  return invoke("send_message", { spaceId, content, msgType });
}

export async function getMessageHistory(
  spaceId: string,
  limit?: number
): Promise<Message[]> {
  return invoke("get_message_history", { spaceId, limit: limit ?? 50 });
}

export async function sendSignal(
  spaceId: string,
  payload: string,
  target?: string
): Promise<void> {
  return invoke("send_signal", { spaceId, payload, target: target ?? null });
}

export async function updateSpaceConfig(spaceId: string, configJson: string): Promise<void> {
  return invoke("update_space_config", { spaceId, configJson });
}

export async function receiveFile(
  spaceId: string,
  fileId: string,
  savePath: string,
  password?: string
): Promise<void> {
  return invoke("receive_file", { spaceId, fileId, savePath, password });
}

export async function sendFile(
  spaceId: string,
  filePath: string,
  password?: string
): Promise<SendFileResult> {
  return invoke("send_file", { spaceId, filePath, password });
}

export async function recordReceivedFile(file: FileInfo): Promise<void> {
  return invoke("record_received_file", { file });
}

export async function deleteFile(spaceId: string, fileId: string): Promise<void> {
  return invoke("delete_file", { spaceId, fileId });
}

export async function listFiles(spaceId: string, limit?: number): Promise<FileInfo[]> {
  return invoke("list_files", { spaceId, limit });
}

export async function getTransferProgress(
  transferId: string
): Promise<FileTransferProgress | null> {
  return invoke("get_transfer_progress", { transferId });
}

export async function getLogs(level?: string, sinceSeq?: number): Promise<LogEntry[]> {
  return invoke("get_logs", { level: level ?? null, sinceSeq: sinceSeq ?? null });
}

export async function getSpaceLogs(spaceId: string, level?: string): Promise<LogEntry[]> {
  return invoke("get_space_logs", { spaceId, level: level ?? null });
}

export async function clearLogs(): Promise<void> {
  return invoke("clear_logs");
}

export async function queryLogs(filter: {
  level?: string;
  space_id?: string;
  module?: string;
  category?: string;
  keyword?: string;
  since_seq?: number;
  before_ts?: string;
  after_ts?: string;
  limit?: number;
}): Promise<LogEntry[]> {
  return invoke("query_logs", {
    level: filter.level ?? null,
    spaceId: filter.space_id ?? null,
    module: filter.module ?? null,
    category: filter.category ?? null,
    keyword: filter.keyword ?? null,
    sinceSeq: filter.since_seq ?? null,
    beforeTs: filter.before_ts ?? null,
    afterTs: filter.after_ts ?? null,
    limit: filter.limit ?? null,
  });
}

export async function exportLogs(filter: {
  level?: string;
  space_id?: string;
  module?: string;
  category?: string;
  keyword?: string;
  before_ts?: string;
  after_ts?: string;
  format?: "txt" | "json";
}): Promise<string> {
  return invoke("export_logs", {
    level: filter.level ?? null,
    spaceId: filter.space_id ?? null,
    module: filter.module ?? null,
    category: filter.category ?? null,
    keyword: filter.keyword ?? null,
    beforeTs: filter.before_ts ?? null,
    afterTs: filter.after_ts ?? null,
    format: filter.format ?? null,
  });
}

export async function getLogModules(): Promise<string[]> {
  return invoke("get_log_modules");
}

export async function clearLogsFiltered(filter: {
  level?: string;
  space_id?: string;
  module?: string;
  category?: string;
  keyword?: string;
}): Promise<void> {
  return invoke("clear_logs_filtered", {
    level: filter.level ?? null,
    spaceId: filter.space_id ?? null,
    module: filter.module ?? null,
    category: filter.category ?? null,
    keyword: filter.keyword ?? null,
  });
}

export async function getLogEnabled(): Promise<boolean> {
  return invoke("get_log_enabled");
}

export async function setLogEnabled(enabled: boolean): Promise<void> {
  return invoke("set_log_enabled", { enabled });
}

export async function getSystemConfig(): Promise<string | null> {
  return invoke("get_system_config");
}

export async function setSystemConfig(config: string): Promise<void> {
  return invoke("set_system_config", { config });
}

export async function getAppConfig(): Promise<Record<string, string>> {
  return invoke("get_app_config");
}

export async function setAppConfig(updates: Record<string, string>): Promise<void> {
  return invoke("set_app_config", { updates });
}

export async function getConfigFilePath(): Promise<string> {
  return invoke("get_config_file_path");
}

export async function getConfigTemplatePath(): Promise<string> {
  return invoke("get_config_template_path");
}

export async function getSpacePeers(spaceId: string): Promise<PeerInfo[]> {
  return invoke<PeerInfo[]>("get_space_peers", { spaceId });
}

export async function addApp(
  spaceId: string,
  name: string,
  options?: {
    category?: string;
    icon?: string;
    protocol?: string;
    hostname?: string;
    port?: string;
    pathname?: string;
  }
): Promise<SpaceApp> {
  return invoke("add_app", {
    spaceId,
    name,
    category: options?.category ?? null,
    icon: options?.icon ?? null,
    protocol: options?.protocol ?? null,
    hostname: options?.hostname ?? null,
    port: options?.port ?? null,
    pathname: options?.pathname ?? null,
  });
}

export async function updateApp(
  appId: string,
  name: string,
  options?: {
    category?: string;
    icon?: string;
    protocol?: string;
    hostname?: string;
    port?: string;
    pathname?: string;
  }
): Promise<void> {
  return invoke("update_app", {
    appId,
    name,
    category: options?.category ?? null,
    icon: options?.icon ?? null,
    protocol: options?.protocol ?? null,
    hostname: options?.hostname ?? null,
    port: options?.port ?? null,
    pathname: options?.pathname ?? null,
  });
}

export async function deleteApp(appId: string): Promise<void> {
  return invoke("delete_app", { appId });
}

export async function listApps(spaceId: string): Promise<SpaceApp[]> {
  return invoke("list_apps", { spaceId });
}

export async function shareApp(appId: string, targetSpaceId: string): Promise<SpaceApp> {
  return invoke("share_app", { appId, targetSpaceId });
}

export async function isDaemonReady(): Promise<boolean> {
  return invoke("is_daemon_ready");
}

export async function getDaemonErrorReason(): Promise<string | null> {
  return invoke("get_daemon_error_reason");
}

export async function getEasyTierVersion(): Promise<string> {
  return invoke("get_easytier_version");
}

export async function checkEasyTierUpdate(): Promise<string[]> {
  return invoke("check_easytier_update");
}

export async function upgradeEasyTierWithProgress(
  version: string,
  useProxy: boolean,
  onProgress: (pct: number) => void,
): Promise<void> {
  const unlisten = await listen<number>("easytier-download-progress", (event) => {
    onProgress(event.payload);
  });
  try {
    await invoke("upgrade_easytier_with_progress", { version, useProxy });
  } finally {
    unlisten();
  }
}

export async function getAclRules(spaceId: string): Promise<AclRule[]> {
  return invoke("get_acl_rules", { spaceId });
}

export async function createAclRule(
  spaceId: string,
  action: string,
  source: string,
  dest: string,
  ports: string,
  description: string
): Promise<AclRule> {
  return invoke("create_acl_rule", { spaceId, action, source, dest, ports, description });
}

export async function updateAclRule(
  spaceId: string,
  ruleId: string,
  action?: string,
  source?: string,
  dest?: string,
  ports?: string,
  description?: string
): Promise<AclRule> {
  return invoke("update_acl_rule", { spaceId, ruleId, action, source, dest, ports, description });
}

export async function deleteAclRule(spaceId: string, ruleId: string): Promise<void> {
  return invoke("delete_acl_rule", { spaceId, ruleId });
}

export async function getPortForwardRules(spaceId: string): Promise<PortForwardRule[]> {
  return invoke("get_port_forward_rules", { spaceId });
}

export async function createPortForwardRule(
  spaceId: string,
  name: string,
  protocol: string,
  sourceIp: string,
  sourcePort: number,
  targetIp: string,
  targetPort: number,
  description: string,
): Promise<PortForwardRule> {
  return invoke("create_port_forward_rule", {
    spaceId,
    name,
    protocol,
    sourceIp,
    sourcePort,
    targetIp,
    targetPort,
    description,
  });
}

export async function updatePortForwardRule(
  spaceId: string,
  ruleId: string,
  name?: string,
  protocol?: string,
  sourceIp?: string,
  sourcePort?: number,
  targetIp?: string,
  targetPort?: number,
  description?: string,
): Promise<PortForwardRule> {
  return invoke("update_port_forward_rule", {
    spaceId,
    ruleId,
    name,
    protocol,
    sourceIp,
    sourcePort,
    targetIp,
    targetPort,
    description,
  });
}

export async function deletePortForwardRule(spaceId: string, ruleId: string): Promise<void> {
  return invoke("delete_port_forward_rule", { spaceId, ruleId });
}
// ---- 配置存储（P2P 分布式配置同步）----

export interface ConfigFileMeta {
  name: string;
  version: number;
  timestamp: number;
  checksum?: string | null;
}

export interface ConfigFile extends ConfigFileMeta {
  content: number[];
}

export async function getConfigVersion(name: string): Promise<ConfigFileMeta | null> {
  return invoke<ConfigFileMeta | null>("get_config_version", { name });
}

export async function downloadConfig(name: string): Promise<ConfigFile | null> {
  return invoke<ConfigFile | null>("download_config", { name });
}

export async function uploadConfig(
  name: string,
  version: number,
  content: string,
  timestamp: number,
): Promise<void> {
  await invoke("upload_config", { name, version, content, timestamp });
}

export async function getRemoteConfigVersion(
  ip: string,
  name: string,
): Promise<ConfigFileMeta | null> {
  return invoke<ConfigFileMeta | null>("get_remote_config_version", { ip, name });
}

export async function downloadRemoteConfig(
  ip: string,
  name: string,
): Promise<ConfigFile | null> {
  return invoke<ConfigFile | null>("download_remote_config", { ip, name });
}

export async function uploadRemoteConfig(
  ip: string,
  name: string,
  version: number,
  content: string,
  timestamp: number,
): Promise<boolean> {
  return invoke<boolean>("store_remote_config", {
    ip,
    file: { name, version, content, timestamp, checksum: null },
  });
}
