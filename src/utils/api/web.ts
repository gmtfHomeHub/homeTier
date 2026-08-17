// src/utils/api/web.ts - Web（服务器）模式实现
import type {
  Space,
  Member,
  Message,
  FileInfo,
  NetworkStats,
  LogEntry,
  SpaceApp,
  SystemApp,
  PeerInfo,
  AclRule,
  PortForwardRule,
  ShareInfo,
  SendFileResult,
  FileTransferProgress,
} from "./core";

// API 基础路径：REST 统一前缀 /api/cmd
const API_BASE = "/api/cmd";

class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method: options.method ?? "GET",
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
    body: options.body,
    credentials: "include",
  });
  if (!res.ok) {
    let msg = `HTTP ${res.status}`;
    try {
      msg = await res.text();
    } catch {
      /* ignore */
    }
    throw new ApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

// ---- 空间 ----
export async function createSpace(
  networkName: string,
  networkSecret: string,
  description?: string
): Promise<Space> {
  return request<Space>("/space/create", {
    method: "POST",
    body: JSON.stringify({ network_name: networkName, network_secret: networkSecret, description }),
  });
}

export async function joinSpace(configJson: string, name?: string): Promise<Space> {
  const config = JSON.parse(configJson) as Record<string, unknown>;
  return request<Space>("/space/join", {
    method: "POST",
    body: JSON.stringify({ ...config, ...(name ? { name } : {}) }),
  });
}

export async function listSpaces(): Promise<Space[]> {
  return request<Space[]>("/space/list");
}

export async function getSpace(spaceId: string): Promise<Space | null> {
  try {
    return await request<Space>(`/space/${spaceId}`);
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return null;
    throw e;
  }
}

export async function deleteSpace(spaceId: string): Promise<void> {
  return request<void>(`/space/${spaceId}`, { method: "DELETE" });
}

export async function leaveSpace(spaceId: string): Promise<void> {
  return request<void>(`/space/${spaceId}/leave`, { method: "POST" });
}

export async function connectSpace(spaceId: string): Promise<void> {
  return request<void>(`/space/${spaceId}/connect`, { method: "POST" });
}

export async function disconnectSpace(spaceId: string): Promise<void> {
  return request<void>(`/space/${spaceId}/disconnect`, { method: "POST" });
}

export async function getSpaceStatus(spaceId: string): Promise<unknown> {
  return request<unknown>(`/space/${spaceId}/status`);
}

export async function listMembers(spaceId: string): Promise<Member[]> {
  return request<Member[]>(`/space/${spaceId}/members`);
}

export async function generateShareLink(spaceId: string, ip?: string): Promise<string> {
  const res = await request<{ link: string }>("/space/share", {
    method: "POST",
    body: JSON.stringify({ space_id: spaceId, ip }),
  });
  return res.link;
}

export async function parseShareLink(link: string): Promise<ShareInfo> {
  const res = await request<ShareInfo>("/space/share/parse", {
    method: "POST",
    body: JSON.stringify({ link }),
  });
  return res;
}

export async function getSpaceConfig(spaceId: string): Promise<string | null> {
  const res = await request<string | null>(`/space/${spaceId}/config`);
  return res ?? null;
}

export async function updateSpaceConfig(spaceId: string, configJson: string): Promise<void> {
  return request<void>(`/space/${spaceId}/config`, {
    method: "POST",
    body: JSON.stringify({ config_json: configJson }),
  });
}

export async function patchSpaceConfig(spaceId: string, patch: Record<string, unknown>): Promise<void> {
  return request<void>(`/space/${spaceId}/config/patch`, {
    method: "POST",
    body: JSON.stringify(patch),
  });
}

// ---- 聊天 ----
export async function sendMessage(
  spaceId: string,
  content: string,
  msgType: string
): Promise<Message> {
  return request<Message>(`/chat/${spaceId}/send`, {
    method: "POST",
    body: JSON.stringify({ content, msg_type: msgType }),
  });
}

export async function getMessageHistory(
  spaceId: string,
  limit?: number
): Promise<Message[]> {
  const params = new URLSearchParams();
  if (limit) params.set("limit", String(limit));
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<Message[]>(`/chat/${spaceId}/history${query}`);
}

// ---- 网络 ----
export async function getNetworkStats(spaceId: string): Promise<NetworkStats> {
  return request<NetworkStats>(`/network/${spaceId}/stats`);
}

export async function getSpacePeers(spaceId: string): Promise<PeerInfo[]> {
  return request<PeerInfo[]>(`/network/${spaceId}/peers`);
}

// ---- 日志 ----
export async function getLogs(level?: string, sinceSeq?: number): Promise<LogEntry[]> {
  const params = new URLSearchParams();
  if (level) params.set("level", level);
  if (sinceSeq !== undefined) params.set("since_seq", String(sinceSeq));
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<LogEntry[]>(`/log/list${query}`);
}

export async function getSpaceLogs(spaceId: string, level?: string): Promise<LogEntry[]> {
  const params = new URLSearchParams();
  if (level) params.set("level", level);
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<LogEntry[]>(`/log/space/${spaceId}${query}`);
}

export async function clearLogs(): Promise<void> {
  return request<void>("/log/clear", { method: "POST" });
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
  const params = new URLSearchParams();
  if (filter.level) params.set("level", filter.level);
  if (filter.space_id) params.set("space_id", filter.space_id);
  if (filter.module) params.set("module", filter.module);
  if (filter.category) params.set("category", filter.category);
  if (filter.keyword) params.set("keyword", filter.keyword);
  if (filter.since_seq !== undefined) params.set("since_seq", String(filter.since_seq));
  if (filter.before_ts) params.set("before_ts", filter.before_ts);
  if (filter.after_ts) params.set("after_ts", filter.after_ts);
  if (filter.limit !== undefined) params.set("limit", String(filter.limit));
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<LogEntry[]>(`/log/query${query}`);
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
  const params = new URLSearchParams();
  if (filter.level) params.set("level", filter.level);
  if (filter.space_id) params.set("space_id", filter.space_id);
  if (filter.module) params.set("module", filter.module);
  if (filter.category) params.set("category", filter.category);
  if (filter.keyword) params.set("keyword", filter.keyword);
  if (filter.before_ts) params.set("before_ts", filter.before_ts);
  if (filter.after_ts) params.set("after_ts", filter.after_ts);
  if (filter.format) params.set("format", filter.format);
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<string>(`/log/export${query}`);
}

export async function getLogModules(): Promise<string[]> {
  return request<string[]>("/log/modules");
}

export async function clearLogsFiltered(filter: {
  level?: string;
  space_id?: string;
  module?: string;
  category?: string;
  keyword?: string;
}): Promise<void> {
  const params = new URLSearchParams();
  if (filter.level) params.set("level", filter.level);
  if (filter.space_id) params.set("space_id", filter.space_id);
  if (filter.module) params.set("module", filter.module);
  if (filter.category) params.set("category", filter.category);
  if (filter.keyword) params.set("keyword", filter.keyword);
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<void>(`/log/clear-filtered${query}`, { method: "POST" });
}

// ---- 配置 ----
export async function getSystemConfig(): Promise<string | null> {
  const res = await request<string | null>("/config/system");
  return res ?? null;
}

export async function setSystemConfig(config: string): Promise<void> {
  return request<void>("/config/system", {
    method: "POST",
    body: JSON.stringify({ config }),
  });
}

export async function getAppConfig(): Promise<Record<string, string>> {
  return request<Record<string, string>>("/config/app");
}

export async function setAppConfig(updates: Record<string, string>): Promise<void> {
  return request<void>("/config/app", {
    method: "POST",
    body: JSON.stringify({ updates }),
  });
}

// ---- 系统信息 ----
export async function getAppVersion(): Promise<{ version: string }> {
  return request<{ version: string }>("/system/version");
}

export async function checkEasyTierBinary(): Promise<{ present: boolean; version?: string }> {
  return request<{ present: boolean; version?: string }>("/system/binary-check");
}

// ---- Proxy ----
export async function getProxyUrl(): Promise<string> {
  const res = await request<{ proxy_url: string }>("/proxy/url");
  return res.proxy_url;
}

export async function getProxyStatus(): Promise<{ running: boolean; port: number; proxy_url: string }> {
  return request<{ running: boolean; port: number; proxy_url: string }>("/proxy/status");
}

export async function registerProxyKey(url: string): Promise<string> {
  const res = await request<{ key: string }>("/proxy/register", {
    method: "POST",
    body: JSON.stringify({ url }),
  });
  return res.key;
}

export async function setProxySource(url: string): Promise<void> {
  return request<void>("/proxy/source", {
    method: "POST",
    body: JSON.stringify({ url }),
  });
}

export async function setDeviceMode(mode: string): Promise<void> {
  await request<void>("/proxy/device", {
    method: "POST",
    body: JSON.stringify({ mode }),
  });
}

export async function getPendingDownloads(): Promise<string[]> {
  const res = await request<{ files: string[] }>("/proxy/downloads");
  return res.files ?? [];
}

// ---- 托盘仅桌面端可用 ----
export async function syncTrayMenu(_spaces: unknown[], _labels: unknown): Promise<void> {
  return Promise.resolve();
}

// ---- WebRTC 信令 ----
export function createSignalConnection(
  spaceId: string,
  onMessage: (msg: unknown) => void,
  onClose: () => void,
  onError: (err: Event) => void
): { send: (msg: unknown) => void; close: () => void } {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${protocol}//${window.location.host}/api/cmd/ws/signal/${spaceId}`);

  ws.onmessage = (event) => {
    try {
      onMessage(JSON.parse(event.data));
    } catch {
      /* ignore */
    }
  };
  ws.onclose = onClose;
  ws.onerror = onError;

  return {
    send: (msg) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(msg));
      }
    },
    close: () => ws.close(),
  };
}

export async function sendSignal(
  spaceId: string,
  payload: string,
  target?: string
): Promise<void> {
  return request<void>(`/space/${spaceId}/signal`, {
    method: "POST",
    body: JSON.stringify({ spaceId, payload, target: target ?? null }),
  });
}

// ---- 文件传输 (服务器中转：raw bytes 上传 + blob 下载) ----

/** Web 模式：file 为浏览器 File 对象（来自文件选择器） */
export async function sendFile(
  spaceId: string,
  file: string | File,
  password?: string
): Promise<SendFileResult> {
  if (typeof file === "string") {
    throw new ApiError(400, "Web 模式不支持文件路径，请通过文件选择器上传");
  }
  const params = new URLSearchParams({ space_id: spaceId, file_name: file.name });
  if (password) params.set("password", password);
  const res = await fetch(`${API_BASE}/file/send?${params.toString()}`, {
    method: "POST",
    headers: { "Content-Type": "application/octet-stream" },
    body: file,
    credentials: "include",
  });
  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }
  return res.json();
}

/** Web 模式：savePath 无意义，浏览器直接下载保存 */
export async function receiveFile(
  spaceId: string,
  fileId: string,
  savePath?: string,
  password?: string
): Promise<void> {
  const res = await fetch(`${API_BASE}/file/${spaceId}/download/${fileId}`, {
    credentials: "include",
  });
  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }
  const blob = await res.blob();
  const cd = res.headers.get("Content-Disposition") || "";
  const m = /filename="([^"]+)"/.exec(cd);
  const name = m ? m[1] : fileId;
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

export async function recordReceivedFile(file: FileInfo): Promise<void> {
  return request<void>("/file/record", {
    method: "POST",
    body: JSON.stringify({ file }),
  });
}

export async function deleteFile(spaceId: string, fileId: string): Promise<void> {
  return request<void>("/file/delete", {
    method: "POST",
    body: JSON.stringify({ spaceId, fileId }),
  });
}

export async function listFiles(spaceId: string, limit?: number): Promise<FileInfo[]> {
  const params = new URLSearchParams();
  if (limit) params.set("limit", String(limit));
  const query = params.toString() ? `?${params.toString()}` : "";
  return request<FileInfo[]>(`/space/${spaceId}/file/list${query}`);
}

export async function getTransferProgress(
  transferId: string
): Promise<FileTransferProgress | null> {
  const params = new URLSearchParams();
  params.set("transferId", transferId);
  return request<FileTransferProgress | null>(`/file/progress?${params.toString()}`);
}

// ---- ACL ----
export async function getAclRules(spaceId: string): Promise<AclRule[]> {
  return request<AclRule[]>(`/space/${spaceId}/acl`);
}

export async function createAclRule(
  spaceId: string,
  action: string,
  source: string,
  dest: string,
  ports: string,
  description: string
): Promise<AclRule> {
  return request<AclRule>(`/space/${spaceId}/acl`, {
    method: "POST",
    body: JSON.stringify({ spaceId, action, source, dest, ports, description }),
  });
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
  return request<AclRule>(`/space/${spaceId}/acl/update`, {
    method: "POST",
    body: JSON.stringify({ spaceId, ruleId, action, source, dest, ports, description }),
  });
}

export async function deleteAclRule(spaceId: string, ruleId: string): Promise<void> {
  return request<void>(`/space/${spaceId}/acl/delete`, {
    method: "POST",
    body: JSON.stringify({ spaceId, ruleId }),
  });
}

// ---- 端口转发 ----
export async function getPortForwardRules(spaceId: string): Promise<PortForwardRule[]> {
  return request<PortForwardRule[]>(`/space/${spaceId}/port-forwards`);
}

export async function createPortForwardRule(
  spaceId: string,
  name: string,
  protocol: string,
  sourceIp: string,
  sourcePort: number,
  targetIp: string,
  targetPort: number,
  description: string
): Promise<PortForwardRule> {
  return request<PortForwardRule>(`/space/${spaceId}/port-forwards`, {
    method: "POST",
    body: JSON.stringify({ spaceId, name, protocol, sourceIp, sourcePort, targetIp, targetPort, description }),
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
  description?: string
): Promise<PortForwardRule> {
  return request<PortForwardRule>(`/space/${spaceId}/port-forwards/update`, {
    method: "POST",
    body: JSON.stringify({ spaceId, ruleId, name, protocol, sourceIp, sourcePort, targetIp, targetPort, description }),
  });
}

export async function deletePortForwardRule(spaceId: string, ruleId: string): Promise<void> {
  return request<void>(`/space/${spaceId}/port-forwards/delete`, {
    method: "POST",
    body: JSON.stringify({ spaceId, ruleId }),
  });
}

// ---- 应用管理 ----
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
  return request<SpaceApp>(`/space/${spaceId}/apps`, {
    method: "POST",
    body: JSON.stringify({ spaceId, name, ...options }),
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
  return request<void>(`/space/${appId}/apps/update`, {
    method: "POST",
    body: JSON.stringify({ appId, name, ...options }),
  });
}

export async function deleteApp(appId: string): Promise<void> {
  return request<void>(`/space/${appId}/apps/delete`, {
    method: "POST",
    body: JSON.stringify({ appId }),
  });
}

export async function listApps(spaceId: string): Promise<SpaceApp[]> {
  return request<SpaceApp[]>(`/space/${spaceId}/apps`);
}

export async function getSystemApps(): Promise<SystemApp[]> {
  return request<SystemApp[]>("/system/apps");
}

export async function shareApp(appId: string, targetSpaceId: string): Promise<SpaceApp> {
  return request<SpaceApp>(`/space/${targetSpaceId}/apps/share`, {
    method: "POST",
    body: JSON.stringify({ appId, target_space_id: targetSpaceId }),
  });
}

// ---- EasyTier 版本 ----
export async function getEasyTierVersion(): Promise<string> {
  const res = await request<{ version: string }>("/system/binary-check");
  return res.version ?? "unknown";
}

export async function checkEasyTierUpdate(): Promise<string[]> {
  return request<string[]>("/easytier/check-update");
}

export async function upgradeEasyTierWithProgress(
  version: string,
  useProxy: boolean,
  _onProgress: (pct: number) => void,
): Promise<void> {
  return request<void>("/easytier/upgrade", {
    method: "POST",
    body: JSON.stringify({ version, useProxy }),
  });
}

// ---- 兼容占位 ----
export async function queryDaemonLogs(): Promise<LogEntry[]> {
  throw new Error("Daemon 日志仅桌面端可用");
}

export async function isDaemonReady(): Promise<boolean> {
  return true;
}

export async function getDaemonErrorReason(): Promise<string | null> {
  return null;
}

export async function getConfigFilePath(): Promise<string> {
  return "/api/cmd/config/path";
}

export async function getConfigTemplatePath(): Promise<string> {
  return "/api/cmd/config/template-path";
}

export async function getLogEnabled(): Promise<boolean> {
  return true;
}

export async function setLogEnabled(_enabled: boolean): Promise<void> {
  return Promise.resolve();
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
  return request<ConfigFileMeta>(`/config-store/${encodeURIComponent(name)}/version`);
}

export async function downloadConfig(name: string): Promise<ConfigFile | null> {
  return request<ConfigFile>(`/config-store/${encodeURIComponent(name)}/download`);
}

export async function uploadConfig(
  name: string,
  version: number,
  content: string,
  timestamp: number,
): Promise<void> {
  await request("/config-store/upload", {
    method: "POST",
    body: JSON.stringify({ name, version, content, timestamp }),
  });
}

export async function getRemoteConfigVersion(
  ip: string,
  name: string,
): Promise<ConfigFileMeta | null> {
  return request<ConfigFileMeta>(
    `/config-store/remote/version?ip=${encodeURIComponent(ip)}&name=${encodeURIComponent(name)}`,
  );
}

export async function downloadRemoteConfig(
  ip: string,
  name: string,
): Promise<ConfigFile | null> {
  return request<ConfigFile>(
    `/config-store/remote/download?ip=${encodeURIComponent(ip)}&name=${encodeURIComponent(name)}`,
  );
}