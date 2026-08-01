pub mod commands;
pub mod chat;
pub mod cleanup;
pub mod config;
pub mod crypto;
pub mod daemon;
pub mod db;
pub mod easytier;
pub mod file;
pub mod log;
pub mod platform;
pub mod proxy;
pub mod screen;
pub mod space;
pub mod types;
pub mod voice;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tauri::{Emitter, Manager};
use proxy::plugins::*;
use proxy::{ActiveOrigin, ProxyHandler, ProxyKeyMap};
use crate::space::manager::SpaceManager;

/// daemon 就绪标志（从后台线程标记，前端通过 Tauri command 轮询）
pub struct DaemonReadyState {
    pub ready: Arc<AtomicBool>,
    pub reason: Arc<std::sync::Mutex<Option<String>>>,
}

/// 全局代理服务器，用于应用退出时关闭
static PROXY_SERVER: OnceLock<Arc<proxy::ProxyServer>> = OnceLock::new();

/// UAC / macOS 提权标记，用于检测当前进程是否通过提权启动
#[cfg(any(target_os = "windows", target_os = "macos"))]
static ELEVATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// 全局 daemon 子进程引用（供 Exit 事件兜底 kill 使用，try_state 在 Exit 时可能失效）
// macOS debug（GUI 非 root）：daemon 经 osascript 提权启动，无 Child 句柄，用 Pid 跟踪；
// 其余场景 daemon 是 GUI 直接子进程，用 Child 跟踪。
#[cfg(not(any(target_os = "android", target_os = "ios")))]
enum DaemonHandle {
    Child(std::process::Child),
    #[cfg(target_os = "macos")]
    Pid(u32),
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl DaemonHandle {
    fn is_alive(&mut self) -> bool {
        match self {
            DaemonHandle::Child(child) => child.try_wait().map(|s| s.is_none()).unwrap_or(false),
            #[cfg(target_os = "macos")]
            DaemonHandle::Pid(pid) => unsafe {
                let ret = libc::kill(*pid as i32, 0);
                if ret == 0 {
                    return true;
                }
                std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            },
        }
    }

    fn pid(&self) -> Option<u32> {
        match self {
            DaemonHandle::Child(child) => Some(child.id()),
            #[cfg(target_os = "macos")]
            DaemonHandle::Pid(pid) => Some(*pid),
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
static DAEMON_CHILD: OnceLock<Arc<std::sync::Mutex<Option<DaemonHandle>>>> = OnceLock::new();

/// 检查当前进程是否以提权模式运行（Windows UAC / macOS）
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn is_elevated_process() -> bool {
    ELEVATED.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> std::process::ExitCode {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());

    let builder = proxy::hometier_protocol::register_protocol(builder);

    let builder = builder.setup(|app| {
            // 应用启动时清空历史日志，确保只记录本次会话的日志
            crate::log::clear();
            log_info!("homeTier 应用启动");

            // 初始化文件日志
            if let Ok(log_dir) = app.path().app_log_dir() {
                crate::log::init_file_logging(&log_dir);
            }

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
                db::Database::new(&db_path).map_err(|e| format!("初始化数据库失败: {}", e))?,
            );
            app.manage(db.clone());

            // 初始化应用配置文件（{app_data_dir}/homeTier.conf）
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let config_path = app_data.join("homeTier.conf");
            let resource_dir = app.path().resource_dir().ok();
            let config_created = crate::config::init(config_path.clone(), resource_dir.as_deref());

            // 首次生成配置文件时，继承 DB 中已有的业务设置（之后以配置文件为准）
            if config_created {
                if let Some(cfg) = crate::config::global() {
                    if let Ok(Some(v)) = db.get_setting("RELAY_NETWORK_PREFIX") {
                        let _ = cfg.set(crate::config::KEY_RELAY_NETWORK_PREFIX, &v);
                    }
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
            crate::log_info!("[GUI] 应用启动，清理临时文件...");
            crate::cleanup::cleanup_all(&easytier_config_dir);
            crate::log_info!("[GUI] 清理完成");
            let _ = std::fs::create_dir_all(&easytier_config_dir);
            let instance_manager = Arc::new(easytier::EasyTierManager::new(easytier_config_dir, app_data.clone()));
            app.manage(instance_manager.clone());
            crate::log_info!("[GUI] EasyTier 管理器已初始化");

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
                crate::log_info!("[GUI] 启动 daemon 子进程...");
                match spawn_daemon(&app_data) {
                    Ok(mut daemon_handle) => {
                        crate::log_info!(format!("[GUI] daemon 已启动, pid={:?}", daemon_handle.pid()));
                        let handle_arc = Arc::new(std::sync::Mutex::new(Some(daemon_handle)));
                        // 存到全局 OnceLock（Exit 事件兜底 kill 用）
                        let _ = DAEMON_CHILD.set(handle_arc.clone());
                        app.manage(handle_arc.clone());
                        // 后台轮询 daemon 就绪（signal 文件 + 进程存活检测）
                        let app_handle = app.handle().clone();
                        let daemon_ready_flag = daemon_ready.clone();
                        let handle_arc_thread = handle_arc.clone();
                        let signal_path = app_data.join("daemon_ready.signal");
                        std::thread::spawn(move || {
                            let daemon_ready_bool = daemon_ready_flag.0;
                            let daemon_ready_reason = daemon_ready_flag.1;
                            for i in 0..60 {
                                // 方式一：检查 signal 文件（daemon bind 成功后写入）
                                if signal_path.exists() {
                                    daemon_ready_bool.store(true, Ordering::SeqCst);
                                    let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": true }));
                                    crate::log_info!("[GUI] daemon 已就绪（signal 文件检测到）");
                                    return;
                                }
                                // 方式二：检查 daemon 进程是否已退出
                                if let Ok(ref mut handle_opt) = handle_arc_thread.lock() {
                                    if let Some(ref mut handle) = handle_opt.as_mut() {
                                        if !handle.is_alive() {
                                            let reason_str = format!("daemon 进程已退出");
                                            crate::log_error!("[GUI] daemon 进程意外退出");
                                            *daemon_ready_reason.lock().unwrap() = Some(reason_str.clone());
                                            let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": false, "reason": reason_str }));
                                            return;
                                        }
                                    }
                                }
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                if i % 10 == 9 {
                                    crate::log_debug!(format!("[GUI] 等待 daemon 就绪中... ({}/60)", i + 1));
                                }
                            }
                            let reason_str = "daemon 启动超时（12s）".to_string();
                            *daemon_ready_reason.lock().unwrap() = Some(reason_str.clone());
                            let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": false, "reason": reason_str }));
                            crate::log_warn!("[GUI] daemon 启动超时");
                        });
                    }
                    Err(e) => {
                        crate::log_error!(format!("[GUI] 启动 daemon 失败: {}", e));
                    }
                }
                // 创建 IPC 客户端供 Tauri 命令使用
                let ipc_client = Arc::new(daemon::client::IpcClient::default_port());
                app.manage(ipc_client);
                crate::log_info!("[GUI] IPC 客户端已创建");

                // 初始化日志转发：GUI 日志 → daemon（单一存储）
                let cached_logs = crate::log::get_all(None);
                std::thread::spawn(move || {
                    let (tx, rx) = std::sync::mpsc::channel::<crate::log::LogEntry>();
                    crate::log::init_forward(tx);
                    // 等待 daemon 就绪
                    let client = crate::daemon::client::IpcClient::default_port();
                    let mut ready = false;
                    for _ in 0..60 {
                        if client.ping_sync() {
                            ready = true;
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                    if !ready { return; }
                    // 同步日志开关到 daemon
                    let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::SetLogEnabled { enabled: crate::log::is_log_enabled() });
                    // Flush 启动前缓存的日志
                    if !cached_logs.is_empty() {
                        let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::WriteLog { entries: cached_logs });
                    }
                    // 持续转发后续 GUI 日志
                    while let Ok(entry) = rx.recv() {
                        let _ = client.send_sync(&crate::daemon::ipc::IpcRequest::WriteLog { entries: vec![entry] });
                    }
                });
            }

            // 初始化空间管理器
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let space_manager = {
                let ipc_client = app
                    .try_state::<Arc<daemon::client::IpcClient>>()
                    .map(|s| s.inner().clone())
                    .ok_or_else(|| "IpcClient state not registered".to_string())?;
                Arc::new(space::manager::SpaceManager::new(db.clone(), instance_manager, ipc_client))
            };
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let space_manager = Arc::new(space::manager::SpaceManager::new(db.clone(), instance_manager));
            let space_manager_clone = space_manager.clone();
            app.manage(space_manager);
            crate::log_info!("[GUI] 空间管理器已创建");

            // 初始化语音管理器
            let voice_manager = voice::engine::VoiceManager::new();
            app.manage(voice_manager);

            // 初始化文件传输管理器
            let file_manager = Arc::new(file::transfer::FileTransferManager::new());
            app.manage(file_manager);

            // 初始化文件服务器注册表（每空间一个 HTTP 文件服务器）
            let file_storage_dir = app_data.join("files");
            let file_registry = Arc::new(file::registry::FileServerRegistry::new(
                file_storage_dir.clone(),
                db.clone(),
            ));
            app.manage(file_registry.clone());
            crate::log_info!("[GUI] 文件服务器注册表已初始化");

            // 后台同步文件服务器状态（随空间连接状态启停）
            {
                let file_registry_sync = file_registry.clone();
                let space_manager_sync = space_manager_clone.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
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
            let screen_share = Arc::new(screen::share::ScreenShareEngine::new());
            app.manage(screen_share);

            // 托盘图标与菜单
            #[cfg(not(target_os = "android"))]
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
                            toggle_window_visibility(&app_handle);
                        }
                        if event.id().as_ref().starts_with("space-") {
                            let space_id = event.id().as_ref().trim_start_matches("space-").to_string();
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
                            toggle_window_visibility(app);
                        }
                    })
                    .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/gray/template.png"))
                        .expect("托盘图标加载失败"))
                        .icon_as_template(true)
                    .build(app)?;
            }

            // 启动 HTTP 代理服务器（用于绕过 iframe 安全限制）
            let active_origin: ActiveOrigin = Arc::new(RwLock::new(None));
            let key_map: ProxyKeyMap = Arc::new(RwLock::new(HashMap::new()));
            let http_forward = Arc::new(HttpForwardPlugin::new(
                key_map.clone(),
                active_origin.clone(),
            ).map_err(|e| format!("创建 HttpForwardPlugin 失败: {}", e))?);
            let handlers: Vec<Arc<dyn ProxyHandler>> = vec![
                Arc::new(HttpsTunnelPlugin),
                http_forward,
            ];

            let proxy_server = Arc::new(proxy::ProxyServer::start(
                vec![
                    Arc::new(CorsPlugin::new()),
                    Arc::new(IframeBypassPlugin),
                ],
                handlers,
            ).map_err(|e| format!("启动代理服务器失败: {}", e))?);
            log_info!(format!("代理服务器已启动: port={}", proxy_server.port));
            proxy::hometier_protocol::set_proxy_port(proxy_server.port);
            let _ = PROXY_SERVER.set(proxy_server.clone());
            app.manage(proxy_server);
            app.manage(key_map);
            app.manage(active_origin);

            // 启动聊天消息监听任务（Desktop）
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let app_handle_clone = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
                    loop {
                        interval.tick().await;

                        // 遍历所有聊天服务器，检查消息队列
                        let servers = space_manager_clone.chat_servers.read().await;
                        for (space_id, server) in servers.iter() {
                            let messages = server.drain_messages().await;
                            for msg in messages {
                                // 验证消息签名
                                let spaces = space_manager_clone.spaces.read().await;
                                if let Some(space) = spaces.iter().find(|s| &s.id == space_id) {
                                    if msg.verify(&space.network_secret) {
                                        // 发送事件到前端
                                        let _ = app_handle_clone.emit("new_message", serde_json::to_value(&msg).unwrap_or_default());
                                    }
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 空间管理
            commands::space::create_space,
            commands::space::join_space,
            commands::space::leave_space,
            commands::space::delete_space,
            commands::space::list_spaces,
            commands::space::remove_member,
            commands::space::list_members,
            commands::space::get_space_config,
            commands::space::update_space_config,
            commands::space::generate_share_link,
            commands::space::parse_share_link,
            commands::space::connect_space,
            commands::space::disconnect_space,
            commands::space::get_space_status,
            commands::space::patch_space_config,
            commands::space::update_local_config,
            // 网络管理
            commands::network::get_network_stats,
            commands::network::update_group_config,
            commands::network::get_space_peers,
            // 聊天
            commands::chat::send_message,
            commands::chat::get_message_history,
            // 信令
            commands::signal::send_signal,
            // 语音
            commands::voice::join_voice_channel,
            commands::voice::leave_voice_channel,
            commands::voice::toggle_mic,
            commands::voice::toggle_speaker,
            // 文件共享
            commands::file::send_file,
            commands::file::receive_file,
            commands::file::list_files,
            commands::file::get_transfer_progress,
            commands::file::record_received_file,
            commands::file::delete_file,
            // 屏幕共享
            commands::screen::start_screen_share,
            commands::screen::stop_screen_share,
            commands::screen::is_screen_sharing,
            commands::screen::get_screen_share_viewers,
            // 工具
            commands::util::get_app_version,
            commands::util::get_system_config,
            commands::util::set_system_config,
            commands::util::get_relay_prefix,
            commands::util::set_relay_prefix,
            commands::util::get_log_enabled,
            commands::util::set_log_enabled,
            // 日志
            commands::log::get_logs,
            commands::log::get_space_logs,
            commands::log::clear_logs,
            // 应用导航
            commands::app::add_app,
            commands::app::update_app,
            commands::app::delete_app,
            commands::app::list_apps,
            // 配置管理
            commands::config::get_app_config,
            commands::config::set_app_config,
            commands::config::get_config_file_path,
            commands::config::get_config_template_path,
            // 代理服务
            commands::proxy::get_proxy_url,
            commands::proxy::get_proxy_status,
            commands::proxy::register_proxy_key,
            commands::proxy::set_proxy_source,
            // 托盘
            commands::tray::update_tray_menu,
            // 守护进程管理
            commands::daemon::is_daemon_ready,
            commands::daemon::get_daemon_error_reason,
            commands::daemon::get_daemon_logs,
            commands::daemon::check_easytier_binary,
            // EasyTier 版本管理
            commands::easytier::get_easytier_version,
            commands::easytier::check_easytier_update,
            commands::easytier::upgrade_easytier,
            commands::easytier::upgrade_easytier_with_progress,
            commands::easytier::build_easytier_from_source,
            // ACL 规则
            commands::network_acls::get_acl_rules,
            commands::network_acls::create_acl_rule,
            commands::network_acls::update_acl_rule,
            commands::network_acls::delete_acl_rule,
            // 端口转发规则
            commands::network_port_forwards::get_port_forward_rules,
            commands::network_port_forwards::create_port_forward_rule,
            commands::network_port_forwards::update_port_forward_rule,
            commands::network_port_forwards::delete_port_forward_rule,
        ])
        .on_window_event(|_win, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = _win.hide();
                api.prevent_close();
            }
        });

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(e) => {
            log_error!(format!("应用构建失败: {}", e));
            return std::process::ExitCode::FAILURE;
        }
    };

    app.run(|app_handle, event| {
        match event {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            tauri::RunEvent::Exit => {
                crate::log_info!("[GUI] 应用退出，开始清理...");

                // 1. 断开所有运行中的空间
                if let Some(space_mgr) = app_handle.try_state::<Arc<SpaceManager>>() {
                    let space_mgr_clone = space_mgr.inner().clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_time()
                            .build()
                            .unwrap();
                        rt.block_on(async {
                            space_mgr_clone.shutdown_all().await;
                        });
                    }).join().ok();
                }

                // 2. 优雅关闭 daemon
                {
                    let client = daemon::client::IpcClient::get_global();
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build();
                    if let Ok(rt) = rt {
                        rt.block_on(async {
                            crate::log_info!("[退出] 发送 IPC Shutdown 到 daemon");
                            let _ = client.shutdown().await;
                        });
                    }
                }

                // 2.5 等待 daemon 退出（最多 8s），给 daemon 留出停止 easytier-core 的时间
                {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                    if let Some(guard) = DAEMON_CHILD.get() {
                        while std::time::Instant::now() < deadline {
                            let exited = guard.lock().ok().map_or(true, |mut g| {
                                g.as_mut().map(|h| !h.is_alive()).unwrap_or(true)
                            });
                            if exited {
                                crate::log_info!("[GUI] daemon 已正常退出");
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                    }
                }

                // 3. 强制终止 daemon 进程（兜底，若仍未退出）
                if let Some(guard) = DAEMON_CHILD.get() {
                    if let Ok(mut handle_opt) = guard.lock() {
                        if let Some(mut handle) = handle_opt.take() {
                            if handle.is_alive() {
                                match handle {
                                    DaemonHandle::Child(mut child) => {
                                        let _ = child.kill();
                                        let _ = child.wait();
                                        crate::log_info!("[GUI] daemon 子进程已强制终止");
                                    }
                                    #[cfg(target_os = "macos")]
                                    DaemonHandle::Pid(pid) => {
                                        // GUI 非 root 无法直接 kill root daemon，经 osascript 提权兜底
                                        crate::log_warn!(format!("[GUI] daemon 未在超时内退出, 尝试 osascript 提权终止 pid={}", pid));
                                        let _ = std::process::Command::new("osascript")
                                            .arg("-e")
                                            .arg(format!(r#"do shell script "kill -9 {}" with administrator privileges"#, pid))
                                            .stdout(std::process::Stdio::null())
                                            .stderr(std::process::Stdio::null())
                                            .spawn();
                                    }
                                }
                            }
                        }
                    }
                }

                // 4. 最终清理（基于 PID 文件关闭残留 easytier-core）
                let easytier_config_dir = app_handle
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("easytier");
                cleanup::cleanup_all(&easytier_config_dir);
                crate::log_info!("[GUI] 应用退出清理完成");
            }
            _ => {}
        }
    });

    // 应用退出时关闭代理服务
    if let Some(proxy) = PROXY_SERVER.get() {
        // Arc 的 Drop 会自动调用 ProxyServer::drop() → shutdown()
        // 此处显式调用以更快关闭
        log_info!("应用退出，代理服务器自动关闭");
    }

    // 应用退出时清空日志
    log::clear();

    std::process::ExitCode::SUCCESS
}

#[cfg(not(target_os = "android"))]
fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true) {
            let _ = window.hide();
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Accessory);
            }
        } else {
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Regular);
            }
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

/// 带参数的入口点，用于 Windows UAC / macOS 提权场景
pub fn run_with_args(elevated: bool) -> std::process::ExitCode {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        ELEVATED.store(elevated, std::sync::atomic::Ordering::SeqCst);
        if elevated {
            log_info!("提权进程已启动");
        }
    }
    run()
}

/// 守护进程入口点（--daemon 模式，路径从 CLI 参数传入）
pub fn run_daemon(config_dir: std::path::PathBuf, data_dir: std::path::PathBuf) -> std::process::ExitCode {
    // daemon 进程也读取同一份应用配置（端口等）。无 AppHandle，resource_dir 传 None
    crate::config::init(data_dir.join("homeTier.conf"), None);
    let rt = tokio::runtime::Runtime::new()
        .expect("创建 tokio 运行时失败");

    let result = rt.block_on(daemon::run_daemon_async(config_dir, data_dir));

    match result {
        Ok(()) => {
            log_info!("守护进程正常退出");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            log_error!("守护进程异常退出: {}", e);
            std::process::ExitCode::FAILURE
        }
    }
}

/// Desktop: 启动 daemon 子进程
/// macOS 且当前进程非 root（debug/dev 模式）：经 osascript 以管理员权限启动 daemon，
/// 使 daemon 获得 root 权限，从而可以终止同样以 root 运行的 easytier-core；
/// 其余场景：直接作为子进程启动。
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn spawn_daemon(data_dir: &std::path::Path) -> Result<DaemonHandle, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    let current_exe = std::env::current_exe().map_err(|e| format!("获取当前可执行文件路径失败: {}", e))?;

    crate::log_info!("[GUI] 启动 daemon 子进程");

    #[cfg(target_os = "macos")]
    {
        let is_root = unsafe { libc::geteuid() == 0 };
        if !is_root {
            // debug/dev 模式：GUI 未提权，经 osascript 以 root 启动 daemon
            crate::log_info!("[GUI] macOS 非 root 环境，经 osascript 提权启动 daemon");
            let log_file = data_dir.join("daemon.log");
            let script_path = std::path::PathBuf::from("/tmp/homeTier-daemon-launch.sh");
            let script_content = format!(
                r#"#!/bin/sh
"{}" --daemon --daemon-config "{}" --daemon-data "{}" < /dev/null > "{}" 2>&1 &
DAEMON_PID=$!
echo "homeTier daemon pid=$DAEMON_PID"
echo "$DAEMON_PID" > "{}/daemon.pid"
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
    [ -f "{}" ] && exit 0
    kill -0 $DAEMON_PID > /dev/null 2>&1 || exit 1
    sleep 1
done
echo "ERROR: homeTier daemon not ready after 30s" >&2
echo "=== daemon.log dump ===" >&2
cat "{}" >&2
exit 1
"#,
                current_exe.display(),
                data_dir.display(),
                data_dir.display(),
                log_file.display(),
                data_dir.display(),
                data_dir.join("daemon_ready.signal").display(),
                log_file.display()
            );

            std::fs::write(&script_path, &script_content)
                .map_err(|e| format!("写入 daemon 启动脚本失败: {}", e))?;

            let escaped_script = script_path.as_path().to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
            let osascript_program = format!(
                "do shell script \"/bin/sh {}\" with administrator privileges",
                escaped_script
            );

            crate::log_info!("[GUI] 正在弹出授权对话框以启动 daemon...");

            let output = Command::new("osascript")
                .arg("-e")
                .arg(&osascript_program)
                .output()
                .map_err(|e| {
                    let msg = format!("macOS 提权启动 daemon 失败: {}", e);
                    crate::log_error!(&msg);
                    msg
                })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            crate::log_info!(format!("[GUI] osascript stdout: {}", stdout.trim()));
            if !stderr_str.is_empty() {
                if stderr_str.contains("User canceled") || stderr_str.contains("canceled") {
                    return Err("用户取消了授权".to_string());
                }
                crate::log_error!(format!("[GUI] osascript stderr: {}", stderr_str));
                crate::log_error!(format!("[GUI] daemon.log 内容: {}",
                    std::fs::read_to_string(&log_file).unwrap_or_else(|_| "(无法读取)".into())
                ));
                return Err(format!("daemon 启动脚本失败: {}", stderr_str));
            }

            if !output.status.success() {
                let log_content = std::fs::read_to_string(&log_file).unwrap_or_else(|_| "(无法读取)".into());
                crate::log_error!(format!("[GUI] daemon.log: {}", log_content));
                return Err(format!("daemon 启动脚本退出码: {}", output.status));
            }

            let daemon_pid = stdout
                .lines()
                .find_map(|l| l.trim().strip_prefix("homeTier daemon pid="))
                .and_then(|s| s.trim().parse::<u32>().ok());
            let pid = daemon_pid.ok_or_else(|| {
                crate::log_error!("[GUI] 未能从 osascript 输出解析 daemon PID");
                "解析 daemon PID 失败".to_string()
            })?;
            crate::log_info!(format!("[GUI] daemon 提权启动成功, pid={}", pid));
            return Ok(DaemonHandle::Pid(pid));
        }
    }

    let mut cmd = Command::new(&current_exe);
    cmd.arg("--daemon")
       .arg("--daemon-config")
       .arg(data_dir)
       .arg("--daemon-data")
       .arg(data_dir)
       .stdout(Stdio::null())
       .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("启动 daemon 失败: {}", e);
        crate::log_error!(&msg);
        msg
    })?;

    // 将 daemon 子进程的 stderr 转发到 GUI 日志
    if let Some(stderr) = child.stderr.take() {
        let reader = std::io::BufReader::new(stderr);
        std::thread::spawn(move || {
            for line in reader.lines() {
                if let Ok(l) = line {
                    crate::log_info!(format!("[Daemon-stderr] {}", l));
                }
            }
        });
    }

    Ok(DaemonHandle::Child(child))
}