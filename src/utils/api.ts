import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Space,
  Member,
  Message,
  FileInfo,
  TransferProgress,
  NetworkStatus,
  ShareInfo,
  LogEntry,
  SpaceApp,
  AuthResult,
  TunStatus,
  TunDeviceInfo,
  PeerInfo,
  AclRule,
  PortForwardRule,
} from "../types";
import type { NetworkConfig } from "../types/network";

// === Space Commands ===

export async function createSpace(name: string, networkSecret: string, ownerId: string, description?: string): Promise<Space> {
  return invoke("create_space", { name, networkSecret, ownerId, description });
}

export async function joinSpace(networkName: string, networkSecret: string): Promise<Space> {
  return invoke("join_space", { networkName, networkSecret });
}

export async function leaveSpace(spaceId: string): Promise<void> {
  return invoke("leave_space", { spaceId });
}

export async function deleteSpace(spaceId: string, callerId?: string): Promise<void> {
  return invoke("delete_space", { spaceId, callerId: callerId ?? "" });
}

export async function listSpaces(): Promise<Space[]> {
  return invoke("list_spaces");
}

export async function generateShareLink(spaceId: string): Promise<string> {
  return invoke("generate_share_link", { spaceId });
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

export async function getSpaceStatus(spaceId: string): Promise<Record<string, unknown> | null> {
  return invoke("get_space_status", { spaceId });
}

export async function patchSpaceConfig(spaceId: string, patch: Record<string, never>): Promise<void> {
  return invoke("patch_space_config", { spaceId, patch });
}

export async function removeMember(spaceId: string, targetMemberId: string, callerId: string): Promise<void> {
  return invoke("remove_member", { spaceId, targetMemberId, callerId });
}

export async function listMembers(spaceId: string): Promise<Member[]> {
  return invoke("list_members", { spaceId });
}

// === Network Commands ===

export async function getNetworkStatus(spaceId: string): Promise<NetworkStatus> {
  return invoke("get_network_status", { spaceId });
}

export async function updateGroupConfig(spaceId: string, config: NetworkConfig): Promise<void> {
  return invoke("update_group_config", { spaceId, config: JSON.stringify(config) });
}

export async function updateLocalConfig(spaceId: string, config: NetworkConfig): Promise<void> {
  return invoke("update_local_config", { spaceId, config: JSON.stringify(config) });
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

// === Voice Commands ===

export async function joinVoiceChannel(spaceId: string): Promise<void> {
  return invoke("join_voice_channel", { spaceId });
}

export async function leaveVoiceChannel(spaceId: string): Promise<void> {
  return invoke("leave_voice_channel", { spaceId });
}

export async function toggleMic(spaceId: string): Promise<boolean> {
  return invoke("toggle_mic", { spaceId });
}

export async function toggleSpeaker(spaceId: string): Promise<boolean> {
  return invoke("toggle_speaker", { spaceId });
}

// === Space Configuration ===

// === File Commands ===

export async function receiveFile(
  fileId: string,
  savePath: string,
  password?: string
): Promise<void> {
  return invoke("receive_file", { fileId, savePath, password });
}

export async function sendFile(
  spaceId: string,
  filePath: string,
  password?: string
): Promise<FileInfo> {
  return invoke("send_file", { spaceId, filePath, password });
}

export async function listFiles(
  spaceId: string,
  limit?: number
): Promise<FileInfo[]> {
  return invoke("list_files", { spaceId, limit });
}

export async function getTransferProgress(
  transferId: string
): Promise<TransferProgress | null> {
  return invoke("get_transfer_progress", { transferId });
}

// === Screen Share Commands ===

export async function startScreenShare(): Promise<void> {
  return invoke("start_screen_share");
}

export async function stopScreenShare(): Promise<void> {
  return invoke("stop_screen_share");
}

export async function isScreenSharing(): Promise<boolean> {
  return invoke("is_screen_sharing");
}

export async function getScreenShareViewers(): Promise<string[]> {
  return invoke("get_screen_share_viewers");
}

// === Hotkey Commands ===

export async function registerHotkey(key: string, action: string): Promise<void> {
  return invoke("register_hotkey", { key, action });
}

export async function unregisterHotkey(key: string): Promise<void> {
  return invoke("unregister_hotkey", { key });
}

// === Events ===

export function onNetworkStatus(callback: (event: NetworkStatus) => void) {
  return listen<NetworkStatus>("network_status", (e) => callback(e.payload));
}

export function onNewMessage(callback: (message: Message) => void) {
  return listen<Message>("new_message", (e) => callback(e.payload));
}

export function onTransferProgress(callback: (progress: TransferProgress) => void) {
  return listen<TransferProgress>("transfer_progress", (e) => callback(e.payload));
}

// === Log Commands ===

export async function getLogs(level?: string, sinceSeq?: number): Promise<LogEntry[]> {
  return invoke("get_logs", { level: level ?? null, sinceSeq: sinceSeq ?? null });
}

export async function getSpaceLogs(spaceId: string, level?: string): Promise<LogEntry[]> {
  return invoke("get_space_logs", { spaceId, level: level ?? null });
}

export async function clearLogs(): Promise<void> {
  return invoke("clear_logs");
}

// === System Config ===

export async function getSystemConfig(): Promise<string | null> {
  return invoke("get_system_config");
}

export async function setSystemConfig(config: string): Promise<void> {
  return invoke("set_system_config", { config });
}

export async function getRelayPrefix(): Promise<string> {
  return invoke("get_relay_prefix");
}

export async function setRelayPrefix(prefix: string): Promise<void> {
  return invoke("set_relay_prefix", { prefix });
}

// === Space Config ===

export async function getSpaceConfig(spaceId: string): Promise<string | null> {
  return invoke("get_space_config", { spaceId });
}

export async function updateSpaceConfig(spaceId: string, configJson: string): Promise<void> {
  return invoke("update_space_config", { spaceId, configJson });
}

// === Peers ===

export async function getSpacePeers(spaceId: string): Promise<PeerInfo[]> {
  return invoke<PeerInfo[]>("get_space_peers", { spaceId });
}

// 这些类型从types导入

// === TUN 授权 ===

export async function getTunStatus(): Promise<TunStatus> {
  return invoke("get_tun_status");
}

export async function refreshTunStatus(): Promise<TunStatus> {
  return invoke("refresh_tun_status");
}

export async function authorizeTun(): Promise<AuthResult> {
  return invoke("authorize_tun");
}

// === TUN 设备管理 ===

export async function createTun(opts: {
  devName?: string;
  ip?: string;
  cidrPrefix?: number;
  mtu?: number;
  routes?: string[];
}): Promise<TunDeviceInfo> {
  return invoke("create_tun", opts);
}

export async function createTunFromFd(opts: {
  fd: number;
  ip?: string;
  cidrPrefix?: number;
  mtu?: number;
}): Promise<TunDeviceInfo> {
  return invoke("create_tun_from_fd", opts);
}

export async function destroyTun(name: string): Promise<void> {
  return invoke("destroy_tun", { name });
}

export async function setTunLinkStatus(name: string, up: boolean): Promise<void> {
  return invoke("set_tun_link_status", { name, up });
}

// === Space Apps ===

export async function addApp(
  spaceId: string,
  name: string,
  callerId: string,
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
    callerId,
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
  callerId: string,
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
    callerId,
    category: options?.category ?? null,
    icon: options?.icon ?? null,
    protocol: options?.protocol ?? null,
    hostname: options?.hostname ?? null,
    port: options?.port ?? null,
    pathname: options?.pathname ?? null,
  });
}

export async function deleteApp(appId: string, callerId: string): Promise<void> {
  return invoke("delete_app", { appId, callerId });
}

export async function listApps(spaceId: string): Promise<SpaceApp[]> {
  return invoke("list_apps", { spaceId });
}

export async function getProxyUrl(): Promise<string> {
  return invoke("get_proxy_url");
}

export async function getProxyStatus(): Promise<{
  running: boolean;
  port: number;
  proxy_url: string;
}> {
  return invoke("get_proxy_status");
}

export async function registerProxyKey(url: string): Promise<string> {
  return invoke("register_proxy_key", { url });
}

export async function setProxySource(url: string): Promise<void> {
  return invoke("set_proxy_source", { url });
}

export async function getWebappMode(): Promise<string> {
  return invoke("get_webapp_mode");
}

export async function setWebappMode(mode: string): Promise<void> {
  return invoke("set_webapp_mode", { mode });
}

export async function openAppView(url: string, x: number, y: number, w: number, h: number): Promise<void> {
  return invoke("open_app_view", { url, x, y, w, h });
}

export async function closeAppView(): Promise<void> {
  return invoke("close_app_view");
}

export async function resizeAppView(x: number, y: number, w: number, h: number): Promise<void> {
  return invoke("resize_app_view", { x, y, w, h });
}

// === Daemon Ready Status ===

export async function isDaemonReady(): Promise<boolean> {
  return invoke("is_daemon_ready");
}

export async function getDaemonErrorReason(): Promise<string | null> {
  return invoke("get_daemon_error_reason");
}

// === Daemon Logs ===

export async function getDaemonLogs(level?: string): Promise<LogEntry[]> {
  return invoke("get_daemon_logs", { level: level ?? null });
}

export async function checkEasytierBinary(): Promise<Record<string, unknown>> {
  return invoke("check_easytier_binary");
}

// === EasyTier Version Management ===

export async function getEasyTierVersion(): Promise<string> {
  return invoke("get_easytier_version");
}

export async function checkEasyTierUpdate(): Promise<string[]> {
  return invoke("check_easytier_update");
}

export async function upgradeEasyTier(version: string, sourcePath?: string): Promise<void> {
  return invoke("upgrade_easytier", { version, sourcePath: sourcePath ?? null });
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

export async function buildEasyTierFromSource(): Promise<string> {
  return invoke("build_easytier_from_source");
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