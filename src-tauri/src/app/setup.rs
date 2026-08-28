//! 应用初始化逻辑（原 lib.rs 中 .setup(|app| { ... }) 闭包体）

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Listener, Manager, async_runtime};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde_json;

use crate::app::daemon::DaemonReadyState;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::app::daemon::set_daemon_child;
use crate::app::PROXY_SERVER;
use crate::{log_info, log_error, log_warn, log_debug};
use crate::proxy::plugins::*;
use crate::proxy::{ActiveOrigin, ProxyHandler, ProxyKeyMap};

/// 应用启动时的完整初始化（绑在 builder.setup 中调用）
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统（桌面端：内存 + 转发 + 可选文件）
    let log_path = app.path().app_log_dir().ok();
    crate::log::init_logger(None, log_path.as_deref(), None);

    crate::log::clear();
    crate::log::restore_history(50_000);
    crate::log_info!("homeTier 应用启动");

    // 初始化数据库
    let db_path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("homeTier.db");
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let db = Arc::new(
        crate::db::Database::new(&db_path).map_err(|e| format!("初始化数据库失败: {}", e))?,
    );
    app.manage(db.clone());
    crate::proxy::hometier_protocol::set_cookie_db(db.clone());

    // 加载自签 CA 证书目录（{app_data_dir}/ca_certs/*.pem），供内网自签证书应用访问
    if let Ok(app_data) = app.path().app_data_dir() {
        let ca_dir = app_data.join("ca_certs");
        let _ = std::fs::create_dir_all(&ca_dir);
        crate::proxy::load_proxy_ca_certs(&ca_dir);

        // 下载目录（代理下载拦截落盘位置）
        let dl_dir = app_data.join("downloads");
        let _ = std::fs::create_dir_all(&dl_dir);
        crate::proxy::hometier_protocol::set_download_dir(&dl_dir.to_string_lossy());
    }

    // 确保本机用户存在（单行 users 表，id=machine_id）
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let user_id = crate::platform::machine_id::get_machine_id()
        .unwrap_or_else(|| format!("machine-{}", hostname));
    if let Err(e) = db.ensure_user(&user_id, &hostname) {
        log_error!("初始化本机用户失败: {}", e);
    }

    // 初始化应用配置文件（{app_data_dir}/homeTier.conf）
    let app_data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let config_path = app_data.join("homeTier.conf");
    // resource_dir 兜底：Tauri 解析失败时退回到当前 exe 所在目录（MSI 把 resources/ 放在 exe 旁）
    let resource_dir = app.path().resource_dir().ok().or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
    });
    let config_created = crate::config::init(config_path.clone(), resource_dir.as_deref());

    // 首次生成配置文件时，继承 DB 中已有的业务设置（之后以配置文件为准）
    if config_created {
        if let Some(cfg) = crate::config::global() {
            if let Ok(Some(v)) = db.get_setting("LOG_ENABLED") {
                let _ = cfg.set(crate::config::KEY_LOG_ENABLED, &v);
            }
        }
    }

    // 启动时应用日志开关（优先配置文件，其次 DB）
    let log_enabled = crate::config::get_bool(
        crate::config::KEY_LOG_ENABLED,
        db.get_setting("LOG_ENABLED")
            .ok()
            .flatten()
            .map(|v| v != "0")
            .unwrap_or(crate::config::DEFAULT_LOG_ENABLED),
    );
    crate::log::set_log_enabled(log_enabled);

    // 后台轮询配置文件热更新（mtime 变化时 reload + 广播 config:changed）
    let app_handle_poll = app.handle().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Some(cfg) = crate::config::global() {
            if cfg.has_external_change() {
                cfg.reload();
                let _ = app_handle_poll.emit("config:changed", ());
            }
        }
    });

    // 初始化 EasyTier 实例管理器
    let easytier_config_dir = app_data.join("easytier");
    log_info!("[GUI] 应用启动，清理临时文件...");
    crate::cleanup::cleanup_all(&app_data, &easytier_config_dir);
    log_info!("[GUI] 清理完成");
    let _ = std::fs::create_dir_all(&easytier_config_dir);
    let instance_manager = Arc::new(crate::easytier::EasyTierManager::new(
        easytier_config_dir, app_data.clone(), resource_dir.as_deref(),
    ));
    app.manage(instance_manager.clone());
    log_info!("[GUI] EasyTier 管理器已初始化");

    // 后台解压内置 easytier-core 二进制（确保版本显示正常、离线可用）
    let mgr = instance_manager.clone();
    async_runtime::spawn(async move {
        if let Err(e) = mgr.downloader.ensure_binary().await {
            crate::log_warn!(format!("[GUI] 内置二进制解压失败（将在线下载）: {}", e));
        } else {
            crate::log_info!("[GUI] 内置 easytier-core 二进制已就绪");
        }
    });

    // daemon 就绪标志（前端通过 Tauri command 轮询）
    let daemon_ready = {
        let ready = Arc::new(AtomicBool::new(false));
        let reason: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
        app.manage(DaemonReadyState { ready: ready.clone(), reason: reason.clone() });
        (ready, reason)
    };

    // Desktop: 启动 daemon 子进程并创建 IPC 客户端
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        log_info!("[GUI] 启动 daemon 子进程...");
        match crate::app::daemon::spawn_daemon(&app_data, resource_dir.as_deref()) {
            Ok(child) => {
                let pid = child.id();
                log_info!(format!("[GUI] daemon 已启动, pid={:?}", pid));
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
                set_daemon_child(child);
                // 后台轮询 daemon 就绪（signal 文件 + 进程存活检测 + 端口连通性验证）
                let app_handle = app.handle().clone();
                let daemon_ready_flag = daemon_ready.clone();
                let signal_path = app_data.join("daemon_ready.signal");
                let state_path = app_data.join("daemon_state.json");
                std::thread::spawn(move || {
                    let daemon_ready_bool = daemon_ready_flag.0;
                    let daemon_ready_reason = daemon_ready_flag.1;
                    for i in 0..60 {
                        // 先检查 daemon 进程是否已退出
                        if let Some(guard) = crate::app::daemon::get_daemon_child() {
                            if let Ok(mut g) = guard.lock() {
                                if g.as_mut().map(|c| !c.is_alive()).unwrap_or(false) {
                                    let reason_str = "daemon 进程已退出".to_string();
                                    log_error!("[GUI] daemon 进程意外退出");
                                    *daemon_ready_reason.lock().unwrap() = Some(reason_str.clone());
                                    let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": false, "reason": reason_str }));
                                    return;
                                }
                            }
                        }
                        // 再检查 signal 文件（仅当 daemon 进程存活时才认）
                        if signal_path.exists() {
                            // 验证端口连通性：读取 daemon_state.json 实际端口并尝试 TCP 连接
                            let mut port_ok = false;
                            if let Ok(content) = std::fs::read_to_string(&state_path) {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                    if let Some(p) = json.get("rpc_port").and_then(|v| v.as_u64()) {
                                        let port = p as u16;
                                        if let Ok(_) = std::net::TcpStream::connect_timeout(
                                            &format!("127.0.0.1:{}", port).parse().unwrap(),
                                            std::time::Duration::from_millis(200),
                                        ) {
                                            port_ok = true;
                                        }
                                    }
                                }
                            }
                            if port_ok {
                                daemon_ready_bool.store(true, Ordering::SeqCst);
                                let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": true }));
                                log_info!("[GUI] daemon 已就绪（signal 文件 + 端口连通性验证通过）");
                                return;
                            } else {
                                log_warn!("[GUI] signal 文件存在但端口不通，等待 daemon 真正就绪...");
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if i % 10 == 9 {
                            log_debug!(format!("[GUI] 等待 daemon 就绪中... ({}/60)", i + 1));
                        }
                    }
                    let reason_str = "daemon 启动超时（12s）".to_string();
                    *daemon_ready_reason.lock().unwrap() = Some(reason_str.clone());
                    let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": false, "reason": reason_str }));
                    log_warn!("[GUI] daemon 启动超时");
                });
            }
            Err(e) => {
                if e.contains("取消") || e.contains("canceled") {
                    log_error!("[GUI] 授权被拒绝，应用退出");
                    app.handle().exit(0);
                    return Ok(());
                }
                log_error!(format!("[GUI] 启动 daemon 失败: {}", e));
            }
        }
        // 创建 IPC 客户端供 Tauri 命令使用（从 daemon_state.json 读取实际端口）
        let ipc_client = Arc::new(crate::daemon::client::IpcClient::from_data_dir(&app_data));
        app.manage(ipc_client);
        log_info!("[GUI] IPC 客户端已创建");

        // 初始化日志转发：GUI 日志 → daemon（单一存储）
        let cached_logs = crate::log::get_all(None);
        let app_data_log = app_data.clone();
        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel::<crate::log::LogEntry>();
            crate::log::init_forward(tx);
            let client = crate::daemon::client::IpcClient::from_data_dir(&app_data_log);
            let mut ready = false;
            for _ in 0..60 {
                if client.ping_sync() {
                    ready = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            if !ready { return; }
            let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::SetLogEnabled { enabled: crate::log::is_log_enabled() });
            if !cached_logs.is_empty() {
                let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::WriteLog { entries: cached_logs });
            }
            while let Ok(entry) = rx.recv() {
                let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::WriteLog { entries: vec![entry] });
            }
        });
    }

    // 初始化空间管理器
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let space_manager = {
        let ipc_client = app
            .try_state::<Arc<crate::daemon::client::IpcClient>>()
            .map(|s| s.inner().clone())
            .ok_or_else(|| "IpcClient state not registered".to_string())?;
        Arc::new(crate::space::manager::SpaceManager::new(db.clone(), instance_manager, ipc_client))
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let space_manager = Arc::new(crate::space::manager::SpaceManager::new(
        db.clone(),
        instance_manager,
        Some(app.handle().clone()),
    ));
    let space_manager_clone = space_manager.clone();
    app.manage(space_manager);
    log_info!("[GUI] 空间管理器已创建");

    // Mobile: VPN event listeners (Kotlin/Swift → Rust → frontend)
    #[cfg(any(target_os = "android", target_os = "ios"))] {
        let app_handle = app.handle().clone();
        let space_manager_vpn = space_manager_clone.clone();
        
        // Listen for vpn:tun-ready event from Kotlin/Swift
        let app_handle_tun = app_handle.clone();
        let space_manager_tun = space_manager_vpn.clone();
        app_handle.listen("vpn:tun-ready", move |event| {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                if let (Some(space_id_val), Some(fd_val)) = (payload.get("spaceId"), payload.get("fd")) {
                    let space_id_str = space_id_val.as_str().map(|s| s.to_string());
                    let fd = fd_val.as_i64();
                    if let (Some(space_id_str), Some(fd)) = (space_id_str, fd) {
                        if let Ok(space_id) = Uuid::parse_str(&space_id_str) {
                            let sm = space_manager_tun.clone();
                            let ah = app_handle_tun.clone();
                            let space_id_clone = space_id_str.clone();
                            tauri::async_runtime::spawn(async move {
                                match sm.set_tun_fd(&space_id, fd as i32) {
                                    Ok(()) => {
                                        crate::log_info!(format!("VPN: TUN fd {} injected for space {}", fd, space_id));
                                        let _ = ah.emit("vpn:state", serde_json::json!({
                                            "spaceId": space_id_clone,
                                            "state": "connected"
                                        }));
                                    }
                                    Err(e) => {
                                        crate::log_error!(format!("VPN: Failed to inject TUN fd: {}", e));
                                        let _ = ah.emit("vpn:state", serde_json::json!({
                                            "spaceId": space_id_clone,
                                            "state": "failed",
                                            "error": e
                                        }));
                                    }
                                }
                            });
                        }
                    }
                }
            }
        });
        
        // Listen for vpn:status-changed event from Kotlin/Swift
        let app_handle_status = app_handle.clone();
        app_handle.listen("vpn:status-changed", move |event| {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                let space_id_str = payload.get("spaceId").and_then(|v| v.as_str()).map(|s| s.to_string());
                let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let error = payload.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                if let Some(space_id_str) = space_id_str {
                    let state = match status {
                        "ready" => "connected",
                        "stopped" => "disconnected",
                        "error" => "failed",
                        _ => "pending-vpn",
                    };
                    
                    let mut json = serde_json::json!({
                        "spaceId": space_id_str,
                        "state": state
                    });
                    if let Some(err) = error {
                        json["error"] = serde_json::Value::String(err);
                    }
                    let _ = app_handle_status.emit("vpn:state", json);
                }
            }
        });
    }

    // 初始化配置存储服务（P2P 分布式配置同步：本地队列 + TCP 监听；移动端不监听端口）
    let config_store_root = app_data.join("config_store");
    let (config_store, queue_receiver) = crate::config_store::ConfigStoreService::new(config_store_root);
    app.manage(config_store.clone());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async_runtime::spawn(async move {
        config_store.start_consumer(queue_receiver);
        if let Err(e) = config_store.serve(crate::config_store::DEFAULT_PORT).await {
            log_error!(format!("[config_store] TCP 服务退出: {}", e));
        }
    });

    // 初始化语音管理器
    let voice_manager = crate::voice::engine::VoiceManager::new();
    app.manage(voice_manager);

    // 初始化移动端语音管理器
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let mobile_voice_state = crate::commands::mobile_voice::MobileVoiceState::new();
        app.manage(mobile_voice_state);
    }

    // 初始化移动端屏幕共享管理器
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let mobile_screen_state = crate::commands::mobile_screen::MobileScreenState::new();
        app.manage(mobile_screen_state);
    }

    // 初始化文件传输管理器
    let file_manager = Arc::new(crate::file::transfer::FileTransferManager::new());
    app.manage(file_manager);

    // 初始化文件服务器注册表
    let file_storage_dir = app_data.join("files");
    let file_registry = Arc::new(crate::file::registry::FileServerRegistry::new(
        file_storage_dir.clone(),
        db.clone(),
    ));
    app.manage(file_registry.clone());
    log_info!("[GUI] 文件服务器注册表已初始化");

    // 后台同步文件服务器状态（随空间连接状态启停）
    {
        let file_registry_sync = file_registry.clone();
        let space_manager_sync = space_manager_clone.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                loop {
                    file_registry_sync.sync(&space_manager_sync).await;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
        });
    }

    // 初始化屏幕共享引擎
    let screen_share = Arc::new(crate::screen::share::ScreenShareEngine::new());
    app.manage(screen_share);

    // 托盘图标与菜单
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use tauri::menu::MenuBuilder;
        let tray_menu = MenuBuilder::new(app)
            .text("show", "显示/隐藏")
            .separator()
            .text("quit", "退出")
            .build()?;
        let app_handle = app.handle().clone();
        let _tray = tauri::tray::TrayIconBuilder::with_id("main")
            .menu(&tray_menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, event| {
                if event.id() == "quit" {
                    app.exit(0);
                }
                if event.id() == "show" {
                    crate::app::window::toggle_window_visibility(&app_handle);
                }
                if event.id().as_ref().starts_with("space-") {
                    let space_id = event.id().as_ref().trim_start_matches("space-").to_string();
                    #[cfg(target_os = "macos")]
                    crate::app::window::activate_main_window(&app_handle);
                    let _ = app.emit("tray-navigate", space_id);
                }
            })
            .on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    button_state: tauri::tray::MouseButtonState::Up,
                    ..
                } = event
                {
                    let app = tray.app_handle();
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::ActivationPolicy;
                        let _ = app.set_activation_policy(ActivationPolicy::Regular);
                        crate::app::window::activate_main_window(app);
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        crate::app::window::toggle_window_visibility(app);
                    }
                }
            })
            .icon(tauri::image::Image::from_bytes(include_bytes!("../../icons/gray/template.png"))
                .expect("托盘图标加载失败"))
            .icon_as_template(true)
            .build(app)?;
    }

    // 启动 HTTP 代理服务器（用于绕过 iframe 安全限制；移动端 webview 直载 dist 无需代理）
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let active_origin: ActiveOrigin = Arc::new(RwLock::new(None));
        let key_map: ProxyKeyMap = Arc::new(RwLock::new(HashMap::new()));
        let http_forward = Arc::new(HttpForwardPlugin::new(
            key_map.clone(),
            active_origin.clone(),
            Some(app.handle().clone()),
        ).map_err(|e| format!("创建 HttpForwardPlugin 失败: {}", e))?);
        let handlers: Vec<Arc<dyn ProxyHandler>> = vec![
            Arc::new(HttpsTunnelPlugin),
            http_forward,
            Arc::new(crate::proxy::plugin::HttpReverseProxyHandler::new()
                .map_err(|e| format!("创建 HttpReverseProxyHandler 失败: {}", e))?),
        ];

        let proxy_server = Arc::new(crate::proxy::ProxyServer::start(
            vec![
                Arc::new(CorsPlugin::new()),
                Arc::new(IframeBypassPlugin),
            ],
            handlers,
            key_map.clone(),
            active_origin.clone(),
        ).map_err(|e| format!("启动代理服务器失败: {}", e))?);
        log_info!(format!("代理服务器已启动: port={}", proxy_server.port));
        crate::proxy::hometier_protocol::set_proxy_port(proxy_server.port);
        let _ = PROXY_SERVER.set(proxy_server.clone());
        app.manage(proxy_server);
        app.manage(key_map);
        app.manage(active_origin);
    }

    // 启动聊天消息监听任务（Desktop）
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let app_handle_clone = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                let servers = space_manager_clone.chat_servers.read().await;
                for (space_id, server) in servers.iter() {
                    let messages = server.drain_messages().await;
                    for msg in messages {
                        let spaces = space_manager_clone.spaces.read().await;
                        if let Some(space) = spaces.iter().find(|s| &s.id == space_id) {
                            if msg.verify(&space.network_secret) {
                                let _ = app_handle_clone.emit("new_message", serde_json::to_value(&msg).unwrap_or_default());
                            }
                        }
                    }
                }
            }
        });
    }

    // [DIAG-GUI-HB] 临时诊断探针：每 5 秒从 GUI 侧探测 daemon 端口与 state 文件
    // 定位问题后整块删除（含上下 [DIAG-GUI-HB] 标记）
    #[cfg(target_os = "windows")]
    {
        let app_data_heartbeat = app_data.clone();
        std::thread::spawn(move || {
            let state_path = app_data_heartbeat.join("daemon_state.json");
            let mut tick = 0u64;
            for _ in 0..u64::MAX {
                tick += 1;
                std::thread::sleep(std::time::Duration::from_secs(5));
                let mut info = format!("[DIAG-Heartbeat-GUI] tick={}", tick);
                let state_content = std::fs::read_to_string(&state_path);
                match state_content {
                    Ok(c) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&c) {
                            if let Some(port) = json.get("rpc_port").and_then(|v| v.as_u64()) {
                                let port16 = port.min(65535) as u16;
                                let alive = std::net::TcpStream::connect_timeout(
                                    &std::net::SocketAddr::from(([127, 0, 0, 1], port16)),
                                    std::time::Duration::from_millis(200),
                                ).is_ok();
                                let pid_val = json.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                                info.push_str(&format!(", port={}, port_alive={}, pid={}", port16, alive, pid_val));
                            }
                        }
                    }
                    Err(_) => info.push_str(", state=MISSING"),
                }
                crate::log_info!(info);
            }
        });
    }

    Ok(())
}