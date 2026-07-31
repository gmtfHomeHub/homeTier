import { v4 as uuidv4 } from 'uuid'

export enum NetworkingMethod {
  PublicServer = 0,
  Manual = 1,
  Standalone = 2,
}

export interface SecureModeConfig {
  enabled: boolean
  // Keep protocol compatibility with backend/import-export flows even though the GUI
  // does not render secure-mode or credential inputs.
  local_private_key?: string
  local_public_key?: string
}

export enum AclProtocol {
  Unspecified = 0,
  TCP = 1,
  UDP = 2,
  ICMP = 3,
  ICMPv6 = 4,
  Any = 5,
}

export enum AclAction {
  Noop = 0,
  Allow = 1,
  Drop = 2,
}

export enum AclChainType {
  UnspecifiedChain = 0,
  Inbound = 1,
  Outbound = 2,
  Forward = 3,
}

export interface AclRule {
  name: string
  description: string
  priority: number
  enabled: boolean
  protocol: AclProtocol
  ports: string[]
  source_ips: string[]
  destination_ips: string[]
  source_ports: string[]
  action: AclAction
  rate_limit: number
  burst_limit: number
  stateful: boolean
  source_groups: string[]
  destination_groups: string[]
}

export interface AclChain {
  name: string
  chain_type: AclChainType
  description: string
  enabled: boolean
  rules: AclRule[]
  default_action: AclAction
}

export interface GroupIdentity {
  group_name: string
  group_secret: string
}

export interface GroupInfo {
  declares: GroupIdentity[]
  members: string[]
}

export interface AclV1 {
  chains: AclChain[]
  group?: GroupInfo
}

export interface Acl {
  acl_v1?: AclV1
}

export interface NetworkConfig {
  instance_id: string

  dhcp: boolean
  virtual_ipv4: string
  network_length: number
  hostname?: string
  network_name: string
  network_secret?: string
  credential_file?: string
  secure_mode?: SecureModeConfig

  networking_method: NetworkingMethod

  public_server_url: string
  peer_urls: string[]

  proxy_cidrs: string[]

  enable_vpn_portal: boolean
  vpn_portal_listen_port: number
  vpn_portal_client_network_addr: string
  vpn_portal_client_network_len: number

  advanced_settings: boolean

  listener_urls: string[]
  latency_first: boolean

  dev_name: string

  use_smoltcp?: boolean
  disable_ipv6?: boolean
  ipv6_public_addr_auto?: boolean
  enable_kcp_proxy?: boolean
  disable_kcp_input?: boolean
  enable_quic_proxy?: boolean
  disable_quic_input?: boolean
  disable_p2p?: boolean
  p2p_only?: boolean
  lazy_p2p?: boolean
  bind_device?: boolean
  no_tun?: boolean
  enable_exit_node?: boolean
  relay_all_peer_rpc?: boolean
  need_p2p?: boolean
  multi_thread?: boolean
  proxy_forward_by_system?: boolean
  disable_encryption?: boolean
  disable_tcp_hole_punching?: boolean
  disable_udp_hole_punching?: boolean
  disable_upnp?: boolean
  enable_udp_broadcast_relay?: boolean
  disable_sym_hole_punching?: boolean

  enable_relay_network_whitelist?: boolean
  relay_network_whitelist: string[]

  enable_manual_routes: boolean
  routes: string[]

  exit_nodes: string[]

  enable_socks5?: boolean
  socks5_port: number

  mtu: number | null
  instance_recv_bps_limit: number | null
  mapped_listeners: string[]

  enable_magic_dns?: boolean
  enable_private_mode?: boolean

  port_forwards: PortForwardConfig[]
  acl?: Acl
}

export function DEFAULT_NETWORK_CONFIG(): NetworkConfig {
  return {
    instance_id: uuidv4(),

    dhcp: true,
    virtual_ipv4: '',
    network_length: 24,
    network_name: 'easytier',
    network_secret: '',
    credential_file: '',

    networking_method: NetworkingMethod.Manual,
    public_server_url: '',
    peer_urls: [],

    proxy_cidrs: [],

    enable_vpn_portal: false,
    vpn_portal_listen_port: 22022,
    vpn_portal_client_network_addr: '',
    vpn_portal_client_network_len: 24,

    advanced_settings: false,

    listener_urls: [
      'tcp://0.0.0.0:11010',
      'udp://0.0.0.0:11010',
      'wg://0.0.0.0:11011',
    ],
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
    enable_magic_dns: false,
    enable_private_mode: false,
    port_forwards: [],
    acl: {
      acl_v1: {
        group: {
          declares: [],
          members: [],
        },
        chains: [],
      },
    },
  }
}

export interface PortForwardConfig {
  bind_ip: string,
  bind_port: number,
  dst_ip: string,
  dst_port: number,
  proto: string
}

// 添加新行
export const addRow = (rows: PortForwardConfig[]) => {
  rows.push({
    proto: 'tcp',
    bind_ip: '',
    bind_port: 65535,
    dst_ip: '',
    dst_port: 65535,
  });
};

// 删除行
export const removeRow = (index: number, rows: PortForwardConfig[]) => {
  rows.splice(index, 1);
};
