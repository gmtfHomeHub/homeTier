// src/services/mobileVpn.ts - Mobile VPN (Android VpnService / iOS NetworkExtension) integration
import { isTauri } from "../utils/api";
import { isMobile } from "../utils/platform";
import * as api from "../utils/api";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// The Tauri plugin name is derived from HomeTierVpnServicePlugin -> "hometiervpnservice"
const PLUGIN = "hometiervpnservice";

export interface VpnConfig {
  spaceId: string;
  networkName: string;
  virtualIp: string;
  virtualIpCidr: number;
  mtu: number;
  routes: string[];
  excludedApps: string[];
  dnsServers: string[];
}

/**
 * Whether mobile VPN is supported on this platform.
 * Android via VpnService. iOS via NetworkExtension (implemented natively).
 */
export async function supportsVpn(): Promise<boolean> {
  if (!isTauri()) return false;
  return isMobile();
}

/**
 * Request VPN authorization from the OS.
 * Returns true if the user granted permission.
 */
export async function prepareVpn(): Promise<boolean> {
  if (!isTauri() || !(await isMobile())) {
    return false;
  }

  try {
    const ret = await invoke<{ granted: boolean }>(
      `plugin:${PLUGIN}|prepare_vpn`,
    );
    return ret?.granted === true;
  } catch (e) {
    console.error("Failed to prepare VPN:", e);
    return false;
  }
}

/**
 * Start the VPN service with the given configuration.
 * The Kotlin VpnService will emit vpn:tun-ready with the tun fd.
 * The Rust backend handles fd injection and emits vpn:state event.
 */
export async function startVpn(config: VpnConfig): Promise<number | null> {
  if (!isTauri() || !(await isMobile())) {
    return null;
  }

  try {
    // Listen for the vpn:state event from Rust (which handles fd injection)
    const unlisten = await listen<{ spaceId: string; state: string; error?: string }>(
      "vpn:state",
      (event) => {
        if (event.payload?.spaceId === config.spaceId) {
          const state = event.payload.state;
          if (state === "connected") {
            // VPN connected successfully, fd was injected by Rust
            stopPromiseResolver?.(0); // fd not needed here, just signal success
          } else if (state === "failed") {
            console.error("VPN connection failed:", event.payload.error);
            stopPromiseResolver?.(null);
          }
        }
      }
    );

    let stopPromiseResolver: ((fd: number | null) => void) | null = null;
    const fdPromise = new Promise<number | null>((resolve) => {
      stopPromiseResolver = (fd: number | null) => {
        resolve(fd);
        stopPromiseResolver = null;
      };
      setTimeout(() => {
        if (stopPromiseResolver) {
          stopPromiseResolver = null;
          resolve(null);
        }
      }, 15000);
    });

    // Start the VPN service via plugin
    await invoke(`plugin:${PLUGIN}|start_vpn`, {
      spaceId: config.spaceId,
      ipv4Addr: `${config.virtualIp}/${config.virtualIpCidr}`,
      routes: config.routes,
      dns: config.dnsServers[0] ?? null,
      disallowedApplications: config.excludedApps,
      mtu: config.mtu,
    });

    const fd = await fdPromise;
    return fd;
  } catch (e) {
    console.error("Failed to start VPN:", e);
    return null;
  }
}

/**
 * Stop the VPN service.
 */
export async function stopVpn(): Promise<boolean> {
  if (!isTauri() || !(await isMobile())) {
    return false;
  }

  try {
    await invoke(`plugin:${PLUGIN}|stop_vpn`);
    return true;
  } catch (e) {
    console.error("Failed to stop VPN:", e);
    return false;
  }
}

/**
 * Get current VPN status.
 */
export async function getVpnStatus(): Promise<{
  running: boolean;
  ipv4Addr: string | null;
  routes: string[];
  dns: string | null;
}> {
  try {
    return await invoke(`plugin:${PLUGIN}|get_vpn_status`);
  } catch {
    return { running: false, ipv4Addr: null, routes: [], dns: null };
  }
}

/**
 * Connect to a space with VPN on mobile.
 * Flow: prepare VPN -> start easytier network -> start VPN -> get fd -> inject fd.
 */
export async function connectWithVpn(
  spaceId: string,
  networkName: string,
  virtualIp: string,
): Promise<boolean> {
  if (!isTauri() || !(await isMobile())) {
    // Desktop: just connect normally
    await api.connectSpace(spaceId);
    return true;
  }

  // 1. Prepare VPN (request authorization if needed)
  const prepared = await prepareVpn();
  if (!prepared) {
    console.error("VPN preparation denied or failed");
    return false;
  }

  // 2. Start EasyTier network first (it waits for the tun fd)
  await api.connectSpace(spaceId);

  // 3. Start VPN service and get fd
  const fd = await startVpn({
    spaceId,
    networkName,
    virtualIp,
    virtualIpCidr: 24,
    mtu: 1500,
    routes: [`${virtualIp.split(".").slice(0, 3).join(".")}.0/24`],
    excludedApps: ["com.hometier.app"],
    dnsServers: [virtualIp],
  });

  if (fd === null) {
    console.error("Failed to get TUN fd");
    await stopVpn();
    return false;
  }

  return true;
}

/**
 * Disconnect space and stop VPN on mobile.
 */
export async function disconnectWithVpn(spaceId: string): Promise<boolean> {
  if (!isTauri() || !(await isMobile())) {
    await api.disconnectSpace(spaceId);
    return true;
  }

  try {
    await api.disconnectSpace(spaceId);
    await stopVpn();
    return true;
  } catch (e) {
    console.error("Failed to disconnect:", e);
    return false;
  }
}
