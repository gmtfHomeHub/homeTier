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
  instance_id?: string;
  instance_name?: string;
  hostname?: string;
  dhcp?: boolean;
  virtual_ipv4?: string;
  network_length?: number;
  network_name: string;
  network_secret: string;
  credential_file?: string;
  networking_method?: string;
  public_server_url?: string;
  peer_urls?: string[];

  ipv4?: string;
  ipv6?: string;
  ipv6_public_addr_auto?: boolean;
  ipv6_public_addr_prefix?: string;

  proxy_cidrs?: string[];

  enable_vpn_portal?: boolean;
  vpn_portal_listen_port?: number;
  vpn_portal_client_network_addr?: string;
  vpn_portal_client_network_len?: number;

  listener_urls?: string[];
  latency_first?: boolean;
  dev_name?: string;

  use_smoltcp?: boolean;
  disable_ipv6?: boolean;
  enable_kcp_proxy?: boolean;
  disable_kcp_input?: boolean;
  enable_quic_proxy?: boolean;
  disable_quic_input?: boolean;
  disable_p2p?: boolean;
  p2p_only?: boolean;
  lazy_p2p?: boolean;
  bind_device?: boolean;
  no_tun?: boolean;
  enable_exit_node?: boolean;
  relay_all_peer_rpc?: boolean;
  need_p2p?: boolean;
  multi_thread?: boolean;
  proxy_forward_by_system?: boolean;
  disable_encryption?: boolean;
  disable_tcp_hole_punching?: boolean;
  disable_udp_hole_punching?: boolean;
  disable_upnp?: boolean;
  enable_udp_broadcast_relay?: boolean;
  disable_sym_hole_punching?: boolean;
  enable_magic_dns?: boolean;
  enable_private_mode?: boolean;

  enable_relay_network_whitelist?: boolean;
  relay_network_whitelist?: string[];

  enable_manual_routes?: boolean;
  routes?: string[];

  exit_nodes?: string[];

  enable_socks5?: boolean;
  socks5_port?: number;

  mtu?: number | null;
  instance_recv_bps_limit?: number | null;
  mapped_listeners?: string[];

  port_forwards?: NetworkPortForwardConfig[];
  acl?: AclConfig;

  // Legacy fields for backward compat
  target_os?: string;
  peers?: PeerConfig[];
  listeners?: string[];
  proxy_networks?: ProxyNetworkConfig[];
  vpn_portal?: VpnPortalConfig;
  flags?: Record<string, string>;
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

export interface NetworkPortForwardConfig {
  proto: string;
  bind_ip: string;
  bind_port: number;
  dst_ip: string;
  dst_port: number;
}

export interface AclConfig {
  acl_v1?: {
    chains?: AclChain[];
    group?: { declares?: { group_name: string; group_secret: string }[]; members?: string[] };
  };
}

export interface AclChain {
  name: string;
  chain_type: number;
  description: string;
  enabled: boolean;
  rules: AclRuleItem[];
  default_action: number;
}

export interface AclRuleItem {
  name: string;
  description: string;
  priority: number;
  enabled: boolean;
  protocol: number;
  ports: string[];
  source_ips: string[];
  destination_ips: string[];
  source_ports: string[];
  action: number;
  rate_limit: number;
  burst_limit: number;
  stateful: boolean;
  source_groups: string[];
  destination_groups: string[];
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

// AclItem interfaces (duplicated — keep the existing AclRule for db compat)
// ... existing AclRule, AclChain, AclRuleItem above

// === Default NetworkConfig factory (reflecting network.ts defaults) ===
export function DEFAULT_NETWORK_CONFIG(): NetworkConfigDetails {
  return {
    instance_id: crypto.randomUUID(),
    dhcp: true,
    virtual_ipv4: '',
    network_length: 24,
    network_name: '',
    network_secret: '',
    credential_file: '',
    networking_method: 'Manual',
    public_server_url: '',
    peer_urls: [],
    proxy_cidrs: [],
    enable_vpn_portal: false,
    vpn_portal_listen_port: 22022,
    vpn_portal_client_network_addr: '',
    vpn_portal_client_network_len: 24,
    listener_urls: ['tcp://0.0.0.0:11010','udp://0.0.0.0:11010','wg://0.0.0.0:11011'],
    latency_first: false,
    dev_name: '',
    use_smoltcp: false,
    disable_ipv6: false,
    ipv6_public_addr_auto: false,
    enable_kcp_proxy: false,
    disable_kcp_input: false,
    enable_quic_proxy: false,
    disable_quic_input: false,
    disable_p2p: false,
    p2p_only: false,
    lazy_p2p: false,
    bind_device: true,
    no_tun: false,
    enable_exit_node: false,
    relay_all_peer_rpc: false,
    need_p2p: false,
    multi_thread: true,
    proxy_forward_by_system: false,
    disable_encryption: false,
    disable_tcp_hole_punching: false,
    disable_udp_hole_punching: false,
    disable_upnp: false,
    enable_udp_broadcast_relay: false,
    disable_sym_hole_punching: false,
    enable_magic_dns: false,
    enable_private_mode: false,
    enable_relay_network_whitelist: false,
    relay_network_whitelist: [],
    enable_manual_routes: false,
    routes: [],
    exit_nodes: [],
    enable_socks5: false,
    socks5_port: 1080,
    mtu: null,
    instance_recv_bps_limit: null,
    mapped_listeners: [],
    port_forwards: [],
    acl: { acl_v1: { chains: [], group: { declares: [], members: [] } } },
  };
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



