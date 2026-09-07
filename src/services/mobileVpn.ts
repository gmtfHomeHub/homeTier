// src/services/mobileVpn.ts - Mobile VPN (Android VpnService / iOS NetworkExtension) integration
import { isTauri } from "../utils/api";
import { isMobile } from "../utils/platform";
import * as api from "../utils/api";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// The Tauri plugin name is derived from HomeTierVpnServicePlugin -> "hometiervpnservice"
const PLUGIN = "hometiervpnservice";

let meshRoutesUnlisten: (() => void) | null = null;
const currentMeshRoutes: Set<string> = new Set();

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

export interface VpnPrepareResult {
  ok: boolean;
  /** 失败原因分类，便于前端给出精确文案 */
  reason?: "invoke_error" | "denied" | "unsupported";
  /** 真实错误详情（invoke 抛错或插件回传），用于定位 */
  detail?: string;
}

/**
 * Request VPN authorization from the OS.
 * Returns a structured result so the real failure mode is not flattened away.
 */
export async function prepareVpn(): Promise<VpnPrepareResult> {
  if (!isTauri() || !(await isMobile())) {
    return { ok: false, reason: "unsupported", detail: "非 Tauri 或非移动端" };
  }

  try {
    const ret = await invoke<{ granted: boolean; error?: string }>(
      `plugin:${PLUGIN}|prepare_vpn`,
    );
    if (ret?.granted === true) {
      return { ok: true };
    }
    // granted=false：授权框被取消，或插件回传了具体错误
    return { ok: false, reason: "denied", detail: ret?.error };
  } catch (e) {
    // invoke 抛错（插件异常/无法启动授权框等）→ 无弹窗场景
    console.error("Failed to prepare VPN:", e);
    return { ok: false, reason: "invoke_error", detail: String(e) };
  }
}

/**
 * Start the VPN service with the given configuration.
 *
 * fd 注入采用双保险：
 * - Rust 侧监听 vpn:tun-ready 事件后注入（setup.rs）
 * - JS 侧同时监听同一事件，收到后直接调用 set_tun_fd 命令兜底
 *   （SpaceManager::set_tun_fd 幂等，重复注入无害）
 * 成功/失败通过 vpn:state 事件回传；总超时 30s。
 */
export async function startVpn(
  config: VpnConfig,
): Promise<{ fd: number | null; error?: string }> {
  if (!isTauri() || !(await isMobile())) {
    return { fd: null, error: "当前平台不支持移动端 VPN" };
  }

  // resolver / promise 先于事件监听创建，避免 TDZ 竞态
  let resolveFd: ((fd: number | null) => void) | null = null;
  const fdPromise = new Promise<number | null>((resolve) => {
    resolveFd = resolve;
  });
  const settle = (fd: number | null) => {
    if (resolveFd) {
      const r = resolveFd;
      resolveFd = null;
      r(fd);
    }
  };

  // 记录真正的失败原因，避免把真实错误吞成通用「VPN connection failed」
  let lastError: string | undefined;

  const unlisteners: Array<() => void> = [];

  try {
    // 1. 监听 Rust 回传的最终状态（connected / failed）
    unlisteners.push(
      await listen<{ spaceId: string; state: string; error?: string }>(
        "vpn:state",
        (event) => {
          if (event.payload?.spaceId !== config.spaceId) return;
          if (event.payload.state === "connected") {
            settle(0); // fd 已由 Rust 注入，此处只需信号成功
          } else if (event.payload.state === "failed") {
            console.error("VPN connection failed:", event.payload.error);
            lastError = event.payload.error || "VPN 服务启动失败";
            settle(null);
          }
        },
      ),
    );

    // 2. 双保险：JS 直接监听 Kotlin 发出的 tun-ready 并注入 fd
    unlisteners.push(
      await listen<{ spaceId: string; fd: number }>(
        "vpn:tun-ready",
        (event) => {
          const p = event.payload;
          if (p?.spaceId !== config.spaceId || typeof p.fd !== "number") return;
          console.log("vpn:tun-ready received, injecting fd directly:", p.fd);
          invoke("set_tun_fd", { spaceId: config.spaceId, fd: p.fd }).catch((e) =>
            console.error("Direct set_tun_fd failed (Rust listener may have handled it):", e),
          );
        },
      ),
    );

    // 3. 总超时 30s（EasyTier 组网需要时间）
    const timer = setTimeout(() => {
      lastError = "VPN 连接超时（30 秒内未建立）";
      settle(null);
    }, 30_000);
    unlisteners.push(() => clearTimeout(timer));

    // 4. 启动 VpnService；need_prepare 时自动重新授权并重试一次
    for (let attempt = 0; attempt < 2; attempt++) {
      const ret = await invoke<{ errorMsg?: string; running?: boolean }>(`plugin:${PLUGIN}|start_vpn`, {
        spaceId: config.spaceId,
        ipv4Addr: `${config.virtualIp}/${config.virtualIpCidr}`,
        routes: config.routes,
        dns: config.dnsServers[0] ?? null,
        disallowedApplications: config.excludedApps,
        mtu: config.mtu,
      });

      if (ret?.errorMsg === "need_prepare" && attempt === 0) {
        const prep = await prepareVpn();
        if (!prep.ok) {
          console.error("VPN re-prepare failed:", prep);
          lastError = prep.reason === "invoke_error"
            ? `VPN 授权请求失败：${prep.detail || "未知错误"}`
            : "VPN 授权被取消";
          settle(null);
          break;
        }
        continue; // 重试启动
      }
      // VPN 已在为同一 space 运行（Kotlin 返回 running: true），无需再次等待 tun-ready
      if (ret?.running === true) {
        console.log("VPN already running for this space, resolving immediately");
        settle(0);
        break;
      }
      break;
    }

    // 5. 等待 fd 注入完成的信号
    const fd = await fdPromise;
    return { fd, error: lastError };
  } catch (e) {
    console.error("Failed to start VPN:", e);
    return { fd: null, error: String(e) };
  } finally {
    for (const un of unlisteners) {
      try {
        un();
      } catch {
        // ignore
      }
    }
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
 *
 * @returns null on success; a non-null string describing the failure reason on failure
 *          (instead of a bare boolean, so the real cause is not swallowed).
 */
export async function connectWithVpn(
  spaceId: string,
  networkName: string,
  virtualIp: string,
): Promise<string | null> {
  if (!isTauri() || !(await isMobile())) {
    // Desktop: just connect normally
    await api.connectSpace(spaceId);
    return null;
  }

  // 1. Prepare VPN (request authorization if needed)
  const prep = await prepareVpn();
  if (!prep.ok) {
    console.error("VPN preparation failed:", prep);
    if (prep.reason === "invoke_error") {
      // 插件未能发起授权（无弹窗场景），给出真实原因而非误导性的“被拒绝”
      return `VPN 授权请求失败：${prep.detail || "未知错误"}`;
    }
    if (prep.reason === "denied") {
      return "VPN 授权被取消";
    }
    return "VPN 授权被拒绝或失败";
  }

  // 2. Start EasyTier network first (it waits for the tun fd)
  // 若上一实例未完全清理导致启动失败，重试一次（等待 2s 后重试）
  let connectErr: string | null = null;
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      await api.connectSpace(spaceId);
      connectErr = null;
      break;
    } catch (e) {
      connectErr = String(e);
      if (attempt === 0) {
        console.warn("connectSpace failed, retrying in 2s:", e);
        await new Promise((r) => setTimeout(r, 2000));
      }
    }
  }
  if (connectErr) {
    return `连接空间失败: ${connectErr}`;
  }

  // 3. Start VPN service and get fd
  const { fd, error } = await startVpn({
    spaceId,
    networkName,
    virtualIp,
    virtualIpCidr: 24,
    mtu: 1500,
    routes: [`${virtualIp.split(".").slice(0, 3).join(".")}.0/24`],
    // 不排除 homeTier 自身：app 内 HTTP 代理需经 VPN TUN 访问虚拟 IP 转发请求。
    // Kotlin VpnService 已移除硬编码 addDisallowedApplication(packageName)。
    excludedApps: [],
    // 不设 DNS：VpnService 仅路由虚拟 IP 子网（非默认网络），
    // 系统 DNS 查询走默认网络（WiFi/蜂窝）的真实 DNS 服务器，不经 TUN。
    // 若设为虚拟 IP，DNS 查询经 TUN 发往虚拟 IP，EasyTier 未启用 magic DNS 会丢弃查询，
    // 导致 reqwest::Client（GaiResolver）解析主机名超时失败。
    dnsServers: [],
  });

  if (fd === null) {
    console.error("Failed to get TUN fd:", error);
    await stopVpn();
    return error || "VPN 连接失败（未获取到 TUN 接口）";
  }

  // 等待 EasyTier 分配虚拟 IP（最多 10s）—— mesh 建连到虚拟 IP 可达有数秒延迟
  const pollStart = Date.now();
  const POLL_MS = 500;
  const MAX_POLL = 10_000;
  while (Date.now() - pollStart < MAX_POLL) {
    const spaces = await api.listSpaces();
    const sp = spaces.find((s) => s.id === spaceId);
    if (sp?.virtual_ip) {
      break;
    }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }

  // 等待 mesh 路由建立：至少有一个 peer 在线（member_count >= 2，含本机）
  // 最多再等 15s，总计约 25s，覆盖 mesh 建连时间
  const meshPollStart = Date.now();
  const MESH_MAX_POLL = 15_000;
  while (Date.now() - meshPollStart < MESH_MAX_POLL) {
    const spaces = await api.listSpaces();
    const sp = spaces.find((s) => s.id === spaceId);
    if (sp && (sp.member_count ?? 0) >= 2) {
      break;
    }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }

  // 显式等待 mesh routes 就绪（轮询 getMeshRoutes 直到非空或超时）
  // 避免 VPN 重启时路由为空，导致无法访问真实局域网 IP
  let meshRoutes: string[] = [];
  const meshRoutesPollStart = Date.now();
  const MESH_ROUTES_MAX_POLL = 10_000;
  const MESH_ROUTES_POLL_MS = 500;
  while (Date.now() - meshRoutesPollStart < MESH_ROUTES_MAX_POLL) {
    try {
      const routes = await api.getMeshRoutes(spaceId);
      if (routes.length > 0) {
        meshRoutes = routes;
        console.log(`Mesh routes 就绪: ${meshRoutes.join(", ")}`);
        break;
      }
    } catch (e) {
      // 忽略暂时性错误，继续轮询
    }
    await new Promise((r) => setTimeout(r, MESH_ROUTES_POLL_MS));
  }
  if (meshRoutes.length === 0) {
    console.warn("Mesh routes 轮询超时，VPN 将仅包含虚拟 IP 子网路由");
  }

  // 构建完整路由列表：虚拟 IP 子网 + 所有 mesh proxy_cidrs
  const fullRoutes = new Set<string>();
  fullRoutes.add(`${virtualIp.split(".").slice(0, 3).join(".")}.0/24`);
  for (const r of meshRoutes) {
    fullRoutes.add(r);
  }

  // 如果 mesh 路由多于初始路由，重启 VPN 以包含所有子网
  // （VpnService 不支持运行时动态添加路由，需重建 session）
  if (fullRoutes.size > 1) {
    console.log(`VPN 路由更新: 添加 mesh 子网代理路由 (${fullRoutes.size} 条)`);
    await stopVpn();
    const { fd: fd2, error: err2 } = await startVpn({
      spaceId,
      networkName,
      virtualIp,
      virtualIpCidr: 24,
      mtu: 1500,
      routes: Array.from(fullRoutes),
      excludedApps: [],
      dnsServers: [],
    });
    if (fd2 === null) {
      console.error("VPN 路由更新失败:", err2);
    }
  }

  // 等待 EasyTier TUN 设备就绪：set_tun_fd 成功后 setup_nic_ctx_for_mobile 异步创建 TUN 设备，
  // 此处额外等待 2s 让 TUN 设备完成初始化，避免代理首次连接时 SYN 丢包。
  await new Promise((r) => setTimeout(r, 2000));

  // 保存当前 mesh 路由集合，用于事件监听时判断是否需要更新
  for (const r of meshRoutes) {
    currentMeshRoutes.add(r);
  }

  // 监听 mesh_routes_updated 事件，动态更新 VPN 路由
  meshRoutesUnlisten = await listen<{ spaceId: string; routes: string[] }>(
    "mesh_routes_updated",
    async (event) => {
      const { spaceId: sid, routes } = event.payload;
      if (sid !== spaceId) return;

      // 检查是否有新路由需要添加
      let hasNewRoutes = false;
      for (const r of routes) {
        if (!currentMeshRoutes.has(r)) {
          hasNewRoutes = true;
          currentMeshRoutes.add(r);
        }
      }

      if (hasNewRoutes) {
        console.log(`收到 mesh 路由更新事件: ${routes.join(", ")}`);
        // 重建完整路由列表并重启 VPN
        const allRoutesList = new Set<string>();
        allRoutesList.add(`${virtualIp.split(".").slice(0, 3).join(".")}.0/24`);
        for (const r of currentMeshRoutes) {
          allRoutesList.add(r);
        }
        console.log(`VPN 路由动态更新: 共 ${allRoutesList.size} 条路由`);
        await stopVpn();
        const { fd: fd3, error: err3 } = await startVpn({
          spaceId,
          networkName,
          virtualIp,
          virtualIpCidr: 24,
          mtu: 1500,
          routes: Array.from(allRoutesList),
          excludedApps: [],
          dnsServers: [],
        });
        if (fd3 === null) {
          console.error("VPN 动态路由更新失败:", err3);
        }
      }
    }
  );

  return null;
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
    // 清理 mesh routes 事件监听
    if (meshRoutesUnlisten) {
      meshRoutesUnlisten();
      meshRoutesUnlisten = null;
    }
    currentMeshRoutes.clear();

    await api.disconnectSpace(spaceId);
    await stopVpn();
    return true;
  } catch (e) {
    console.error("Failed to disconnect:", e);
    return false;
  }
}
