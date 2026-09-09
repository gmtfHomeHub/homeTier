// src/services/mobileVpn.ts - Mobile VPN (Android VpnService / iOS NetworkExtension) integration
import { isTauri } from "../utils/api";
import { isMobile } from "../utils/platform";
import * as api from "../utils/api";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Space } from "../types";

// The Tauri plugin name is derived from HomeTierVpnServicePlugin -> "hometiervpnservice"
const PLUGIN = "hometiervpnservice";

const IPV4_RE = /^\d{1,3}(\.\d{1,3}){3}$/;

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

function isValidIpv4(s: string | undefined | null): boolean {
  if (!s) return false;
  const trimmed = s.trim();
  if (!IPV4_RE.test(trimmed)) return false;
  const octets = trimmed.split(".").map(Number);
  return octets.every((o) => o >= 0 && o <= 255);
}

/**
 * 解析移动端本机虚拟 IP（VPN 接口地址必须等于 EasyTier 节点身份 IP，否则 mesh
 * L3 回包黑洞）。规则：
 * 1. 空间配置为静态 IP（dhcp=false 且 virtual_ipv4 合法）→ 直接使用；
 * 2. 否则（DHCP 模式或无 IP）→ 分配一个确定性 IP 并写回配置（dhcp=false），
 *    保证下次连接一致，且 VPN 接口地址与节点身份一致。
 *
 * Android VpnService 必须在 establish() 前定死接口地址，因此移动端不支持 DHCP。
 */
export async function resolveVirtualIpForConnect(space: Space): Promise<string> {
  let cfg: Record<string, unknown> | null = null;
  try {
    cfg = space.config_json ? (JSON.parse(space.config_json) as Record<string, unknown>) : null;
  } catch {
    cfg = null;
  }

  const rawIp =
    cfg && typeof cfg.virtual_ipv4 === "string" && cfg.virtual_ipv4.trim()
      ? (cfg.virtual_ipv4 as string)
      : cfg && typeof cfg.ipv4 === "string"
        ? (cfg.ipv4 as string)
        : "";
  const plainIp = rawIp.split("/")[0].trim();

  // 配置里已有合法 IP（无论 dhcp 标记）→ 以它为准（to_easytier_config 在 IP 存在时
  // 也会强制 dhcp=false，节点身份即此 IP）；顺带把 dhcp 标记纠正落库。
  if (isValidIpv4(plainIp)) {
    if (cfg && cfg.dhcp !== false) {
      try {
        const newCfg: Record<string, unknown> = { ...cfg, dhcp: false };
        await api.updateSpaceConfig(space.id, JSON.stringify(newCfg));
      } catch (e) {
        console.error("纠正 dhcp 标记失败（不影响本次连接）:", e);
      }
    }
    return plainIp;
  }

  // 无静态 IP（DHCP 或从未配置）→ 分配确定性 IP 并写回（仅当已有合法 config_json，
  // 避免用残缺对象覆盖完整配置）。Android VpnService 必须先于 DHCP 定址，移动端只能静态。
  const baseParts = isValidIpv4(plainIp) ? plainIp.split(".") : ["10", "144", "144"];
  let h = 0;
  for (const ch of space.id) {
    h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  }
  const allocated = `${baseParts.slice(0, 3).join(".")}.${(h % 250) + 2}`;

  if (cfg) {
    try {
      const newCfg: Record<string, unknown> = { ...cfg, dhcp: false, virtual_ipv4: allocated };
      if (typeof newCfg.ipv4 === "string") {
        newCfg.ipv4 = allocated;
      }
      await api.updateSpaceConfig(space.id, JSON.stringify(newCfg));
      console.log(`已为本机分配静态虚拟 IP: ${allocated}（已写回空间配置）`);
    } catch (e) {
      console.error("写回静态虚拟 IP 失败（本次仍按该 IP 连接）:", e);
    }
  }
  return allocated;
}

let meshRoutesUnlisten: (() => void) | null = null;

/**
 * Connect to a space with VPN on mobile.
 *
 * 时序（修复要点）：
 * 1. prepare VPN；
 * 2. 探测本机物理 LAN 子网（排除集，防止 VPN 捕获本地直连流量）；
 * 3. 【提前】注册 mesh_routes_updated 监听（首事件不再丢失）；
 * 4. connectSpace 启动 EasyTier 实例；
 * 5. 先以最小路由（虚拟子网）建立 VPN 并注入 fd —— mesh 只有拿到 tun fd 才能组网；
 * 6. 组网后拉取一次 get_mesh_routes 与事件缓冲合并，若路由集合有变化则
 *    仅重建一次 VPN（整条 VpnService 以新路由重建 + 注入新 fd，EasyTier 支持新 fd 重新绑 NICI）；
 * 7. 之后路由变化（peer 加入/宣告代理子网）同样走重建，集合不变时不动作。
 *
 * @returns null 表示成功；否则返回失败原因
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
      return `VPN 授权请求失败：${prep.detail || "未知错误"}`;
    }
    if (prep.reason === "denied") {
      return "VPN 授权被取消";
    }
    return "VPN 授权被拒绝或失败";
  }

  // 2. 自动探测物理 LAN 子网（本机直连可达网段，始终不进 VPN 路由）
  const localSubnets: string[] = [];
  try {
    const result = await invoke<{ subnets: string[] }>(`plugin:${PLUGIN}|detect_lan_subnets`);
    const found = result?.subnets || [];
    for (const s of found) {
      if (!localSubnets.includes(s)) localSubnets.push(s);
    }
    if (localSubnets.length > 0) {
      console.log("自动探测到物理 LAN 子网:", localSubnets);
    } else {
      console.warn("自动探测未发现物理 LAN 子网（接口枚举为空）");
    }
  } catch (e) {
    const msg = String(e);
    console.error("物理 LAN 子网探测 invoke 失败:", msg);
  }

  const virtualIpClean = virtualIp.split("/")[0].trim();
  const virtualIpSubnet = `${virtualIpClean.split(".").slice(0, 3).join(".")}.0/24`;
  const ipv4Ok = isValidIpv4(virtualIpClean);
  if (!ipv4Ok) {
    return `本机虚拟 IP 非法: ${virtualIp}`;
  }

  // 3. 【提前】注册 mesh_routes_updated，首事件不再因监听晚到而丢失
  const currentMeshRoutes: Set<string> = new Set();
  // peer 虚拟 IP（/32）：mesh 中每个节点可属不同网段，仅路由本机 /24 无法覆盖；
  // 每次重建前刷新一次，保证新加入/离开节点都能进/出 VPN 路由。
  let peer32s: Set<string> = new Set();
  let vpnStarted = false; // 初始 VPN 建立前，mesh 事件只记录、不触发重建
  let rebuildBusy = false;
  let rebuildQueued = false;
  let appliedRoutes: Set<string> | null = null;

  const refreshPeer32s = async (): Promise<void> => {
    try {
      const peers = await api.getSpacePeers(spaceId);
      const next = new Set<string>();
      for (const p of peers || []) {
        const ip = p?.virtual_ip?.split("/")[0]?.trim();
        if (isValidIpv4(ip)) next.add(`${ip}/32`);
      }
      peer32s = next;
    } catch (e) {
      console.error("getSpacePeers 刷新 peer /32 失败:", e);
    }
  };

  const desiredRoutes = (): Set<string> => {
    const desired = new Set<string>([virtualIpSubnet]);
    for (const r of currentMeshRoutes) {
      desired.add(r);
    }
    for (const r of peer32s) {
      desired.add(r);
    }
    // 本机物理接口可达网段绝不被 VPN 捕获（本地直连优先）
    for (const local of localSubnets) {
      desired.delete(local);
    }
    return desired;
  };
  const setsEqual = (a: Set<string>, b: Set<string>) =>
    a.size === b.size && Array.from(a).every((x) => b.has(x));

  /** 仅当路由集合发生变化时，整条重建 VpnService（新 fd 注入由 tun-ready 双保险完成） */
  const rebuildVpn = async (): Promise<void> => {
    if (!vpnStarted) return; // 初始 VPN 尚未建立，事件只进缓冲
    if (rebuildBusy) {
      rebuildQueued = true;
      return;
    }
    rebuildBusy = true;
    try {
      while (true) {
        rebuildQueued = false;
        await refreshPeer32s();
        const desired = desiredRoutes();
        if (appliedRoutes && setsEqual(appliedRoutes, desired)) {
          return; // 集合无变化，不动 VPN
        }
        console.log(
          `VPN 路由更新重建: ${Array.from(desired).join(", ")}` +
            (localSubnets.length > 0 ? `（排除本地: ${localSubnets.join(", ")}）` : ""),
        );
        await stopVpn();
        const { fd, error } = await startVpn({
          spaceId,
          networkName,
          virtualIp: virtualIpClean,
          virtualIpCidr: 24,
          mtu: 1500,
          routes: Array.from(desired),
          excludedApps: [],
          dnsServers: [],
        });
        if (fd === null) {
          console.error("VPN 路由更新重建失败:", error);
          appliedRoutes = null;
          return;
        }
        appliedRoutes = desired;
        if (!rebuildQueued) return;
      }
    } finally {
      rebuildBusy = false;
    }
  };

  // 3a. 注册监听（必须在 connectSpace 之前，保证首帧事件被记录）
  if (meshRoutesUnlisten) {
    meshRoutesUnlisten();
    meshRoutesUnlisten = null;
  }
  meshRoutesUnlisten = await listen<{ spaceId: string; routes: string[] }>(
    "mesh_routes_updated",
    async (event) => {
      const { spaceId: sid, routes } = event.payload;
      if (sid !== spaceId) return;
      const next = new Set(routes || []);
      const changed =
        next.size !== currentMeshRoutes.size ||
        Array.from(next).some((x) => !currentMeshRoutes.has(x));
      if (changed) {
        currentMeshRoutes.clear();
        for (const r of next) currentMeshRoutes.add(r);
        console.log(`收到 mesh 路由更新事件: ${(routes || []).join(", ")}`);
        await rebuildVpn();
      }
    },
  );

  // 失败路径统一清理监听
  const cleanupListener = () => {
    if (meshRoutesUnlisten) {
      meshRoutesUnlisten();
      meshRoutesUnlisten = null;
    }
  };

  // 4. Start EasyTier network first (it waits for the tun fd)
  let connectErr: string | null = null;
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      await api.connectSpace(spaceId, localSubnets);
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
    cleanupListener();
    return `连接空间失败: ${connectErr}`;
  }

  // 5. 先建立最小 VPN（仅虚拟 IP 子网），拿到 tun fd 供 EasyTier 组网
  const { fd, error } = await startVpn({
    spaceId,
    networkName,
    virtualIp: virtualIpClean,
    virtualIpCidr: 24,
    mtu: 1500,
    routes: [virtualIpSubnet],
    excludedApps: [],
    dnsServers: [],
  });

  if (fd === null) {
    console.error("Failed to get TUN fd:", error);
    await stopVpn();
    cleanupListener();
    return error || "VPN 连接失败（未获取到 TUN 接口）";
  }
  vpnStarted = true;
  appliedRoutes = new Set([virtualIpSubnet]);

  // 6. 等待 EasyTier 分配/上报虚拟 IP（最多 10s）
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

  // 7. 等待 TUN 就绪 + mesh 路由首轮采集（poll 前 10s 每 500ms 快扫）
  await new Promise((r) => setTimeout(r, 2000));

  // 8. 兜底回放：直接拉一次当前 mesh 路由，与事件缓冲对齐后做最终一次路由收敛。
  //    修复「首个非空事件在监听注册前发出而永久丢失」的时序缺陷。
  try {
    const current = await api.getMeshRoutes(spaceId);
    const next = new Set(current || []);
    const changed =
      next.size !== currentMeshRoutes.size ||
      Array.from(next).some((x) => !currentMeshRoutes.has(x));
    if (changed) {
      currentMeshRoutes.clear();
      for (const r of next) currentMeshRoutes.add(r);
      console.log(`回放当前 mesh 路由: ${(current || []).join(", ")}`);
    }
  } catch (e) {
    console.error("getMeshRoutes 回放失败:", e);
  }
  await rebuildVpn();

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

    await api.disconnectSpace(spaceId);
    await stopVpn();
    return true;
  } catch (e) {
    console.error("Failed to disconnect:", e);
    return false;
  }
}
