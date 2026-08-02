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
} from "../types";
import type { NetworkConfig } from "../types/network";

// === Space Commands ===

export async function createSpace(name: string, networkSecret: string, description?: string): Promise<Space> {
  return invoke("create_space", { name, networkSecret, description });
}

/** 加入空间：configJson 为含 network_name/network_secret 等字段的部分 easytier 配置 json，
 *  缺省字段由后端按默认值补全后落库 */
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

export interface TraySpace {
  id: string;
  name: string;
}

export interface TrayLabels {
  show: string;
  quit: string;
}

export async function syncTrayMenu(spaces: TraySpace[], labels: TrayLabels): Promise<void> {
  return invoke("update_tray_menu", { spaces, labels });
}

// === Network Commands ===

export async function getNetworkStats(spaceId: string): Promise<NetworkStats> {
  return invoke<NetworkStats>("get_network_stats", { spaceId });
}

// === Chat Commands ===

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

// === Space Configuration ===

// === File Commands ===

export interface SendFileResult {
  transfer_id: string;
  file_info: FileInfo;
}

export interface FileTransferProgress {
  transfer_id: string;
  file_name: string;
  bytes_transferred: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  status: "Transferring" | "Paused" | "Completed" | "Failed";
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

export async function recordReceivedFile(
  file: FileInfo
): Promise<void> {
  return invoke("record_received_file", { file });
}

export async function deleteFile(
  spaceId: string,
  fileId: string
): Promise<void> {
  return invoke("delete_file", { spaceId, fileId });
}

export async function listFiles(
  spaceId: string,
  limit?: number
): Promise<FileInfo[]> {
  return invoke("list_files", { spaceId, limit });
}

export async function getTransferProgress(
  transferId: string
): Promise<FileTransferProgress | null> {
  return invoke("get_transfer_progress", { transferId });
}

// === Screen Share Commands ===

// === Events ===

export async function getLogs(level?: string, sinceSeq?: number): Promise<LogEntry[]> {
  return invoke("get_logs", { level: level ?? null, sinceSeq: sinceSeq ?? null });
}

export async function getSpaceLogs(spaceId: string, level?: string): Promise<LogEntry[]> {
  return invoke("get_space_logs", { spaceId, level: level ?? null });
}

export async function clearLogs(): Promise<void> {
  return invoke("clear_logs");
}

export async function getLogEnabled(): Promise<boolean> {
  return invoke("get_log_enabled");
}

export async function setLogEnabled(enabled: boolean): Promise<void> {
  return invoke("set_log_enabled", { enabled });
}

// === System Config ===

export async function getSystemConfig(): Promise<string | null> {
  return invoke("get_system_config");
}

export async function setSystemConfig(config: string): Promise<void> {
  return invoke("set_system_config", { config });
}

// === App Config (配置文件) ===

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

// === Space Config ===

export async function updateSpaceConfig(spaceId: string, configJson: string): Promise<void> {
  return invoke("update_space_config", { spaceId, configJson });
}

// === Peers ===

export async function getSpacePeers(spaceId: string): Promise<PeerInfo[]> {
  return invoke<PeerInfo[]>("get_space_peers", { spaceId });
}

// === Space Apps ===

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

// === Daemon Ready Status ===

export async function isDaemonReady(): Promise<boolean> {
  return invoke("is_daemon_ready");
}

export async function getDaemonErrorReason(): Promise<string | null> {
  return invoke("get_daemon_error_reason");
}

// === EasyTier Version Management ===

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

// === ACL Rules ===

export async function getAclRules(spaceId: string): Promise<AclRule[]> {
  return invoke("get_acl_rules", { spaceId });
}

export async function createAclRule(spaceId: string, action: string, source: string, dest: string, ports: string, description: string): Promise<AclRule> {
  return invoke("create_acl_rule", { spaceId, action, source, dest, ports, description });
}

export async function updateAclRule(spaceId: string, ruleId: string, action?: string, source?: string, dest?: string, ports?: string, description?: string): Promise<AclRule> {
  return invoke("update_acl_rule", { spaceId, ruleId, action, source, dest, ports, description });
}

export async function deleteAclRule(spaceId: string, ruleId: string): Promise<void> {
  return invoke("delete_acl_rule", { spaceId, ruleId });
}

// === Port Forward Rules ===

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
  return invoke("create_port_forward_rule", { spaceId, name, protocol, sourceIp, sourcePort, targetIp, targetPort, description });
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
  return invoke("update_port_forward_rule", { spaceId, ruleId, name, protocol, sourceIp, sourcePort, targetIp, targetPort, description });
}

export async function deletePortForwardRule(spaceId: string, ruleId: string): Promise<void> {
  return invoke("delete_port_forward_rule", { spaceId, ruleId });
}