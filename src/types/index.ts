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

export interface NetworkConfig extends EasyTierConfig {}

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

export interface SpaceApp {
  id: string;
  space_id: string;
  name: string;
  category: string;
  icon: string;
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
  if (app.pathname) parts.push(`/${app.pathname.replace(/^\//, '')}`);
  return parts.join('') || app.protocol;
}

