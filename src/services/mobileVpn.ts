// src/services/mobileVpn.ts - Mobile VPN (Android VpnService / iOS NetworkExtension) integration
import { isTauri } from "../utils/api";
import { isMobile } from "../utils/platform";
import * as api from "../utils/api";
import { listen, emit } from "@tauri-apps/api/event";

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
 * Request VPN authorization from the OS.
 * Returns true if the user granted permission.
 */
export async function prepareVpn(): Promise<boolean> {
  if (!isTauri() || !(await isMobile())) {
    return false;
  }

  try {
    // For Android: VpnService.prepare(context) returns an Intent that needs to be started for result
    // For iOS: NEVPNManager.shared().loadFromPreferences then saveToPreferences
    // This is handled by the native VpnService via a Tauri event
    const prepared = await new Promise<boolean>((resolve) => {
      let resolved = false;
      const timer = setTimeout(() => {
        if (!resolved) {
          resolved = true;
          resolve(false);
        }
      }, 5000);

      listen<{ prepared: boolean }>("vpn:prepared", (event) => {
        if (!resolved) {
          resolved = true;
          clearTimeout(timer);
          resolve(event.payload.prepared);
        }
      });

      emit("vpn:prepare-request").catch(() => {});
    });

    return prepared;
  } catch (e) {
    console.error("Failed to prepare VPN:", e);
    return false;
  }
}

/**
 * Start the VPN service with the given configuration.
 * Returns the file descriptor on success.
 */
export async function startVpn(config: VpnConfig): Promise<number | null> {
  if (!isTauri() || !(await isMobile())) {
    return null;
  }

  try {
    const result = await new Promise<{ fd: number; success: boolean; error?: string }>(
      (resolve) => {
        let resolved = false;
        const timer = setTimeout(() => {
          if (!resolved) {
            resolved = true;
            resolve({ fd: -1, success: false, error: "VPN start timeout" });
          }
        }, 10000);

        listen<{ fd: number; success: boolean; error?: string }>("vpn:tun-ready", (event) => {
          if (!resolved) {
            resolved = true;
            clearTimeout(timer);
            resolve(event.payload);
          }
        });

        emit("vpn:start", { config }).catch(() => {});
      },
    );

    if (result.success && result.fd >= 0) {
      return result.fd;
    }
    console.error("VPN start failed:", result.error);
    return null;
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
    await emit("vpn:stop");
    return true;
  } catch (e) {
    console.error("Failed to stop VPN:", e);
    return false;
  }
}

/**
 * Connect to a space with VPN on mobile.
 * Flow: prepare VPN -> start VPN -> get fd -> set fd on easytier -> connect.
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

  // 2. Start VPN service and get fd
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
    return false;
  }

  // 3. Start EasyTier network
  await api.connectSpace(spaceId);

  // 4. Inject TUN fd
  try {
    await api.setTunFd(spaceId, fd);
  } catch (e) {
    console.error("Failed to inject TUN fd:", e);
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
