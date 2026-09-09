// src-tauri/src/daemon/peer_routes.rs
// 桌面端对端虚拟 IP 自动路由（路线 B）
//
// 背景：easytier 的 OS 路由只自动包含「本机 virtual_ipv4 所在子网」与「对端宣告的
// proxy_cidrs」；对端精确虚拟 IP 只在 easytier 内部路由表（ipv4_peer_id_map），不会进
// OS 路由表。因此 mesh 内存在跨 /24 的虚拟地址时（例如本机 10.144.144.x、对端
// 122.23.44.x），发起方 OS 没有把包送进 tun 的路由，导致「无代理配置无法访问对端」。
//
// 本模块由 daemon（root/提权上下文）在连接生命周期内，把每个已连接对端的虚拟 IP 以
// /32 精确路由维护到 easytier 的 tun 设备上（与移动端 VpnService 的 peer32s 同构），
// 随对端上下线增删，从而恢复「无代理即点即开」的桌面行为。

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::easytier::EasyTierManager;

const SYNC_INTERVAL_MS: u64 = 2000;
const CMD_TIMEOUT_SECS: u64 = 5;

/// 控制单个空间连接期间的 peer 路由同步任务
pub struct PeerRouteSync {
    easytier: Arc<EasyTierManager>,
    handle: Mutex<Option<tokio::task::AbortHandle>>,
}

impl PeerRouteSync {
    pub fn new(easytier: Arc<EasyTierManager>) -> Self {
        Self {
            easytier,
            handle: Mutex::new(None),
        }
    }

    /// 空间连接成功后启动路由同步（幂等：先中止旧任务再启动）
    pub async fn start(&self, space_id: Uuid) {
        {
            let mut h = self.handle.lock().await;
            if let Some(old) = h.take() {
                old.abort();
            }
        }
        crate::log_info!(format!("[PeerRoutes] 启动对端路由同步, space_id={}", space_id), &space_id.to_string());
        println!("[PeerRoutes] 启动对端路由同步, space_id={}", space_id);
        let easytier = self.easytier.clone();
        let handle = tokio::spawn(async move {
            sync_peer_routes(easytier, space_id).await;
        });
        *self.handle.lock().await = Some(handle.abort_handle());
    }

    /// 断开空间时中止路由同步（tun 接口随实例删除，其上路由自动消失，无需逐个删除）
    pub async fn stop(&self) {
        let mut h = self.handle.lock().await;
        if let Some(old) = h.take() {
            old.abort();
            crate::log_debug!("[PeerRoutes] 已中止路由同步任务");
            println!("[PeerRoutes] 已中止路由同步任务");
        }
    }
}

fn valid_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            p.len() >= 1 && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit()) && p.parse::<u32>().map(|v| v <= 255).unwrap_or(false)
        })
}

/// 执行一条系统命令，返回 (成功?, 输出摘要)
async fn run_cmd(prog: &str, args: &[String]) -> (bool, String) {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(CMD_TIMEOUT_SECS),
        tokio::process::Command::new(prog).args(args).output(),
    )
    .await;
    match result {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let summary = format!("{}{}", stderr.trim(), stdout.trim());
            (out.status.success(), summary)
        }
        Ok(Err(e)) => (false, format!("启动命令失败 {}: {}", prog, e)),
        Err(_) => (false, format!("命令超时 {}: {}", prog, args.join(" "))),
    }
}

/// 查找拥有指定 IP 的网络接口（即 easytier 的 tun），失败返回 None
async fn discover_tun_ifname(self_ip: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let (ok, out) = run_cmd("/sbin/ifconfig", &["-l".into()]).await;
        if !ok || out.is_empty() {
            crate::log_warn!(format!("[PeerRoutes] ifconfig -l 失败: {}", out));
            return None;
        }
        for name in out.split_whitespace() {
            let (ok2, detail) = run_cmd("/sbin/ifconfig", &[name.to_string()]).await;
            if !ok2 {
                continue;
            }
            let needle = format!(" inet {} ", self_ip);
            let has = detail
                .lines()
                .any(|l| l.trim_start().starts_with("inet") && l.split_whitespace().nth(1) == Some(self_ip))
                || detail.contains(&needle);
            if has {
                return Some(name.to_string());
            }
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        let (ok, out) = run_cmd("ip", &["-4".into(), "-o".into(), "addr".into(), "show".into()]).await;
        if !ok {
            crate::log_warn!(format!("[PeerRoutes] ip addr show 失败: {}", out));
            return None;
        }
        for line in out.lines() {
            // 示例: 2: eth0    inet 10.144.144.99/24 brd 10.144.144.255 scope global eth0
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 4 {
                continue;
            }
            let ifname = tokens[1].to_string();
            for (i, tok) in tokens.iter().enumerate() {
                if *tok == "inet" {
                    if let Some(cidr) = tokens.get(i + 1) {
                        let ip = cidr.split('/').next().unwrap_or("");
                        if ip == self_ip {
                            return Some(ifname);
                        }
                    }
                }
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = self_ip;
        None
    }
}

/// 增加一条主机路由（幂等：已存在视为成功）
async fn add_host_route(ip: &str, ifname: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        let (ok, out) = run_cmd("/sbin/route", &["-n".into(), "add".into(), "-host".into(), ip.into(), "-interface".into(), ifname.into()]).await;
        if ok || out.contains("exists") || out.contains("File exists") {
            return true;
        }
        crate::log_warn!(format!("[PeerRoutes] route add 失败 {} via {}: {}", ip, ifname, out));
        false
    }
    #[cfg(target_os = "linux")]
    {
        let (ok, out) = run_cmd("ip", &["route".into(), "add".into(), format!("{}/32", ip), "dev".into(), ifname.into()]).await;
        if ok || out.contains("File exists") {
            return true;
        }
        crate::log_warn!(format!("[PeerRoutes] ip route add 失败 {} dev {}: {}", ip, ifname, out));
        false
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (ip, ifname);
        false
    }
}

/// 删除主机路由（不存在视为成功）
async fn del_host_route(ip: &str, ifname: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        let (ok, out) = run_cmd("/sbin/route", &["-n".into(), "delete".into(), "-host".into(), ip.into(), "-interface".into(), ifname.into()]).await;
        if ok || out.contains("not in table") || out.contains("No such process") {
            return true;
        }
        crate::log_warn!(format!("[PeerRoutes] route delete 失败 {} via {}: {}", ip, ifname, out));
        false
    }
    #[cfg(target_os = "linux")]
    {
        let (ok, out) = run_cmd("ip", &["route".into(), "del".into(), format!("{}/32", ip), "dev".into(), ifname.into()]).await;
        if ok || out.contains("No such process") || out.contains("not found") {
            return true;
        }
        crate::log_warn!(format!("[PeerRoutes] ip route del 失败 {} dev {}: {}", ip, ifname, out));
        false
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (ip, ifname);
        false
    }
}

/// 周期性同步：把已连接对端的虚拟 IP 以 /32 路由指向 easytier tun
async fn sync_peer_routes(easytier: Arc<EasyTierManager>, space_id: Uuid) {
    crate::log_info!(format!("[PeerRoutes] 同步任务运行中, space_id={}", space_id), &space_id.to_string());

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        crate::log_warn!("[PeerRoutes] 当前平台暂不支持自动对端路由（仅 macOS/Linux），跨网段节点请配置子网代理");
        return;
    }

    let mut applied: HashSet<String> = HashSet::new();
    let mut last_self_ip: Option<String> = None;
    let mut ifname: Option<String> = None;

    loop {
        // 实例不可达（easytier-core 重启/停止）时静默重试；由外部 stop() 负责真正中止
        let peers = match easytier.get_peers(&space_id).await {
            Ok(p) => p,
            Err(_) => {
                applied.clear();
                ifname = None;
                last_self_ip = None;
                continue;
            }
        };

        // 本机虚拟 IP（用于定位 tun 接口与排除自身）
        let self_ip = peers
            .iter()
            .find(|p| p.is_local)
            .and_then(|p| p.virtual_ip.clone())
            .filter(|ip| valid_ipv4(ip));

        let Some(self_ip) = self_ip else {
            continue; // DHCP/虚拟 IP 尚未分配，下一轮再试
        };

        // 本机 IP 变化（重连/DHCP 重新分配）时重新发现接口并清空已加路由
        if last_self_ip.as_deref() != Some(self_ip.as_str()) {
            ifname = discover_tun_ifname(&self_ip).await;
            applied.clear();
            last_self_ip = Some(self_ip.clone());
            if ifname.is_none() {
                crate::log_warn!(format!("[PeerRoutes] 未找到持有本机 IP {} 的 tun 接口，等待重试", self_ip), &space_id.to_string());
                continue;
            }
        }
        let Some(ifname) = ifname.clone() else {
            continue;
        };

        // 期望集合：所有对端虚拟 IP（排除自身）
        let mut desired: HashSet<String> = HashSet::new();
        for p in peers.iter() {
            if p.is_local {
                continue;
            }
            if let Some(ip) = p.virtual_ip.as_deref() {
                if valid_ipv4(ip) && ip != self_ip {
                    desired.insert(ip.to_string());
                }
            }
        }

        // 增删差异
        let to_add: Vec<String> = desired.difference(&applied).cloned().collect();
        let to_del: Vec<String> = applied.difference(&desired).cloned().collect();

        for ip in &to_del {
            if del_host_route(ip, &ifname).await {
                applied.remove(ip);
            }
        }
        for ip in &to_add {
            if add_host_route(ip, &ifname).await {
                applied.insert(ip.clone());
            }
        }
        if !to_add.is_empty() || !to_del.is_empty() {
            let mut sorted: Vec<String> = desired.iter().cloned().collect();
            sorted.sort();
            crate::log_info!(format!(
                "[PeerRoutes] 路由同步完成, space_id={}, 已加={:?}, 已删={:?}, 当前对端={:?}",
                space_id, to_add, to_del, sorted
            ), &space_id.to_string());
            println!(
                "[PeerRoutes] 路由同步完成, space_id={}, 已加={:?}, 已删={:?}, 当前对端={:?}",
                space_id, to_add, to_del, sorted
            );
        }

        tokio::time::sleep(std::time::Duration::from_millis(SYNC_INTERVAL_MS)).await;
    }
}
