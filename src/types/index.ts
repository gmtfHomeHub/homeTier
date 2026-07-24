import { EasyTierConfig } from './config';

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
  status: "disconnected" | "connecting" | "connected";
  virtual_ip?: string;
  member_count: number;
  config_json?: string;
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

export interface TransferProgress {
  transfer_id: string;
  file_name: string;
  bytes_transferred: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  status: "transferring" | "paused" | "completed" | "failed";
}

export interface NetworkStatus {
  space_id: string;
  status: string;
  virtual_ip?: string;
  latency_ms?: number;
  connected_peers: number;
}

export interface ShareInfo {
  network_name: string;
  network_secret: string;
  host_hint?: string;
}

export interface EasyTierNetworkConfig extends EasyTierConfig {
  instance_id?: string;
  instance_name?: string;
}

// === Detailed Network Configuration ===
export interface NetworkConfigDetails {
  target_os?: string;
  instance_name?: string;
  hostname?: string;
  ipv4?: string;
  ipv6?: string;
  dhcp?: boolean;
  ipv6_public_addr_provider?: boolean;
  ipv6_public_addr_auto?: boolean;
  ipv6_public_addr_prefix?: string;
  network_name: string;
  network_secret: string;
  networking_method?: string;
  peers: PeerConfig[];
  listeners: string[];
  mapped_listeners: string[];
  proxy_networks: ProxyNetworkConfig[];
  routes: string[];
  exit_nodes: string[];
  vpn_portal?: VpnPortalConfig;
  port_forwards: PortForwardConfig[];
  flags: Record<string, string>;
  file_logger?: LogConfig;
  console_logger?: LogConfig;
}

export interface PeerConfig {
  uri: string;
  peer_public_key?: string;
}

export interface ProxyNetworkConfig {
  cidr: string;
  mapped_cidr?: string;
  allow?: string[];
}

export interface PortForwardConfig {
  bind_addr: string;
  dst_addr: string;
  proto: string;
}

export interface VpnPortalConfig {
  client_cidr: string;
  wireguard_listen: string;
}

export interface LogConfig {
  level?: string;
  file?: string;
  dir?: string;
  size_mb?: number;
  count?: number;
}

export interface LogEntry {
  timestamp: string;
  level: "debug" | "info" | "warning" | "error";
  module: string;
  message: string;
  space_id?: string;
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

export interface AuthResult {
  success: boolean;
  message: string;
  needs_restart: boolean;
}

export interface TunStatus {
  tun_available: boolean;
  platform: string;
  elevated: boolean;
}

export interface TunDeviceInfo {
  name: string;
  ip: string | null;
  mtu: number;
  platform: string;
  fd: number | null;
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

export function buildAppUrlDisplay(app: SpaceApp): string {
  const parts: string[] = [];
  if (app.hostname) parts.push(app.hostname);
  if (app.port) parts.push(`:${app.port}`);
  const pathname = app.pathname ? `/${app.pathname.replace(/^\//, '')}` : '';
  return parts.length > 0 ? `${parts.join('')}${pathname}` : pathname;
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

// 网络配置扩展
export interface NetworkConfigDetails {
  space_id: string;
  acl_rules?: AclRule[];
  port_forward_rules?: PortForwardRule[];
}

