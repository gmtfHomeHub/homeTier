import { SpaceStatus } from '../enum';
export interface Space {
  id: string;
  name: string;
  description?: string;
  owner_id?: string;
  network_name: string;
  network_secret: string;
  created_at: string;
  last_connected_at?: string;
  is_auto_connect: boolean;
  status: SpaceStatus;
  virtual_ip?: string;
  member_count: number;
  config_json?: string;
}

/** 分享链接信息（后端解密后返回） */
export interface ShareInfo {
  network_name: string;
  network_secret: string;
  host_hint?: string;
  virtual_ip?: string;
  dhcp?: boolean;
  peer_urls?: string[];
  listener_urls?: string[];
}

export interface Member {
  id: string;
  space_id: string;
  nickname: string;
  virtual_ip?: string;
  is_online: boolean;
  is_owner: boolean;
  joined_at: string;
  last_seen_at?: string;
}

export interface Message {
  id: string;
  space_id: string;
  sender_id: string;
  sender_name: string;
  msg_type: "text" | "image" | "system";
  content: string;
  timestamp: string;
  status: "sending" | "sent" | "delivered" | "failed";
}

export interface FileInfo {
  id: string;
  space_id: string;
  sender_id: string;
  file_name: string;
  file_size: number;
  file_hash?: string;
  mime_type?: string;
  is_compressed: boolean;
  is_password_protected: boolean;
  storage_path?: string;
  created_at: string;
}

export interface NetworkStats {
  rx_bytes: number;
  tx_bytes: number;
  rx_packets: number;
  tx_packets: number;
  loss_rate: number;
  avg_latency_ms: number;
}

export interface LogEntry {
  seq: number;
  timestamp: string;
  level: "debug" | "info" | "warning" | "error";
  target: string;
  module: string;
  category:
    | "system"
    | "network"
    | "webrtc"
    | "data"
    | "proxy"
    | "daemon"
    | "space"
    | "server";
  message: string;
  space_id?: string;
  trace_id?: string;
}

export type LogLevel = "debug" | "info" | "warning" | "error";
export type LogCategory =
  | "system"
  | "network"
  | "webrtc"
  | "data"
  | "proxy"
  | "daemon"
  | "space"
  | "server";

export interface LogFilter {
  level?: LogLevel;
  space_id?: string;
  module?: string;
  category?: LogCategory;
  keyword?: string;
  since_seq?: number;
  limit?: number;
}

export interface PeerInfo {
  peer_id: number;
  virtual_ip?: string;
  hostname?: string;
  latency_ms?: number;
  loss_rate?: number;
  rx_bytes?: number;
  tx_bytes?: number;
  connected: boolean;
  is_local: boolean;
  version?: string;
  tunnel_proto?: string;
  nat_type?: string;
}

export interface SpaceApp {
  id: string;
  space_id: string;
  name: string;
  category: string;
  icon: string;
  description?: string;
  protocol: string;
  hostname: string;
  port: string;
  pathname: string;
  sort_order: number;
  created_by: string;
  created_at: string;
}

export function buildAppUrl(app: SpaceApp): string {
  const base = `${app.protocol}//${app.hostname}`;
  const port = app.port ? `:${app.port}` : '';
  const path = app.pathname ? `/${app.pathname.replace(/^\//, '')}` : '';
  return `${base}${port}${path}`;
}

// ACL 规则类型
export interface AclRule {
  id: string;
  space_id: string;
  action: "allow" | "deny";
  source: string; // CIDR 格式或 "any"
  dest: string;   // CIDR 格式或 "any"
  ports: string;  // 单个端口、端口范围或 "any"
  description: string;
  created_at: string;
  updated_at: string;
}

// 端口转发规则类型
export interface PortForwardRule {
  id: string;
  space_id: string;
  name: string;
  protocol: "tcp" | "udp";
  source_ip: string;
  source_port: number;
  target_ip: string;
  target_port: number;
  description: string;
  created_at: string;
  updated_at: string;
}



