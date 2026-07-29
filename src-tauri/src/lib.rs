pub mod commands;
pub mod chat;
pub mod cleanup;
pub mod daemon;
pub mod db;
pub mod easytier;
pub mod file;
pub mod hotkey;
pub mod log;
pub mod platform;
pub mod proxy;
pub mod screen;
pub mod space;
pub mod types;
pub mod tun;
pub mod voice;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tauri::{Emitter, Manager};

use commands::app_view::AppWebview;
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

/// 管理 daemon 子进程生命周期，在 app 退出时自动清理
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DaemonGuard(pub Arc<std::sync::Mutex<Option<std::process::Child>>>);

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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_os::init());

    let builder = proxy::hometier_protocol::register_protocol(builder);

    let builder = builder.setup(|app| {
            log_info!("homeTier 应用启动");

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

            // 初始化 EasyTier 实例管理器
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let easytier_config_dir = app_data.join("easytier");
            crate::log_info!("[GUI] 应用启动，开始清理遗留进程...");
            crate::cleanup::startup_precheck(&easytier_config_dir);
            crate::log_info!("[GUI] 清理完成");
            let _ = std::fs::create_dir_all(&easytier_config_dir);
            let instance_manager = Arc::new(easytier::EasyTierManager::new(easytier_config_dir, app_data));
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
                match spawn_daemon() {
                    Ok(mut child) => {
                        crate::log_info!("[GUI] daemon 子进程已启动");
                        let child_arc = Arc::new(std::sync::Mutex::new(Some(child)));
                        app.manage(crate::DaemonGuard(child_arc.clone()));
                        // 后台轮询 daemon 就绪（signal 文件 + 子进程存活检测）
                        let app_handle = app.handle().clone();
                        let daemon_ready_flag = daemon_ready.clone();
                        let child_arc_thread = child_arc.clone();
                        let signal_path = crate::daemon::ipc::get_signal_path();
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
                                // 方式二：检查子进程是否已退出
                                if let Ok(ref mut child_opt) = child_arc_thread.lock() {
                                    if let Some(ref mut child) = child_opt.as_mut() {
                                        match child.try_wait() {
                                            Ok(Some(status)) => {
                                                let reason_str = format!("daemon 进程退出: {}", status);
                                                crate::log_error!(format!("[GUI] daemon 子进程意外退出: {:?}", status));
                                                *daemon_ready_reason.lock().unwrap() = Some(reason_str.clone());
                                                let _ = app_handle.emit("daemon-ready", serde_json::json!({ "ready": false, "reason": reason_str }));
                                                return;
                                            }
                                            Ok(None) => {}
                                            Err(e) => {
                                                crate::log_error!(format!("[GUI] try_wait 失败: {}", e));
                                            }
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
            }

            // 初始化空间管理器
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let space_manager = {
                let ipc_client = app
                    .try_state::<Arc<daemon::client::IpcClient>>()
                    .map(|s| s.inner().clone())
                    .ok_or_else(|| "IpcClient state not registered".to_string())?;
                Arc::new(space::manager::SpaceManager::new(db, instance_manager, ipc_client))
            };
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let space_manager = Arc::new(space::manager::SpaceManager::new(db, instance_manager));
            let space_manager_clone = space_manager.clone();
            app.manage(space_manager);
            crate::log_info!("[GUI] 空间管理器已创建");

            // 初始化语音管理器
            let voice_manager = voice::engine::VoiceManager::new();
            app.manage(voice_manager);

            // 初始化快捷键管理器
            let hotkey_manager = hotkey::platform::HotkeyManager::new();
            hotkey_manager.init(app.handle());
            app.manage(hotkey_manager);

            // 初始化文件传输管理器
            let file_manager = Arc::new(file::transfer::FileTransferManager::new());
            app.manage(file_manager);

            // 初始化屏幕共享引擎
            let screen_share = Arc::new(screen::share::ScreenShareEngine::new());
            app.manage(screen_share);

            // 初始化 TUN 能力检查
            platform::init_tun_cap_check();

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
            app.manage(AppWebview(std::sync::Mutex::new(None)));

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
            commands::network::get_network_status,
            commands::network::get_network_stats,
            commands::network::update_group_config,
            commands::network::get_space_peers,
            // 聊天
            commands::chat::send_message,
            commands::chat::get_message_history,
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
            // 屏幕共享
            commands::screen::start_screen_share,
            commands::screen::stop_screen_share,
            commands::screen::is_screen_sharing,
            commands::screen::get_screen_share_viewers,
            // 快捷键
            commands::hotkey::register_hotkey,
            commands::hotkey::unregister_hotkey,
            commands::hotkey::list_hotkeys,
            // 工具
            commands::util::get_app_version,
            commands::util::get_system_config,
            commands::util::set_system_config,
            commands::util::get_relay_prefix,
            commands::util::set_relay_prefix,
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
            commands::config::get_effective_config,
            // 代理服务
            commands::proxy::get_proxy_url,
            commands::proxy::get_proxy_status,
            commands::proxy::register_proxy_key,
            commands::proxy::set_proxy_source,
            // WebView 模式
            commands::util::get_webapp_mode,
            commands::util::set_webapp_mode,
            commands::util::get_tun_status,
            commands::util::refresh_tun_status,
            commands::util::authorize_tun,
            commands::tun::create_tun,
            commands::tun::create_tun_from_fd,
            commands::tun::destroy_tun,
            commands::tun::set_tun_link_status,
            commands::app_view::open_app_view,
            commands::app_view::close_app_view,
            commands::app_view::resize_app_view,
            // 守护进程管理
            commands::daemon::check_daemon_running,
            commands::daemon::get_daemon_status,
            commands::daemon::is_daemon_ready,
            commands::daemon::get_daemon_error_reason,
            commands::daemon::daemon_connect_space,
            commands::daemon::daemon_disconnect_space,
            commands::daemon::daemon_list_spaces,
            commands::daemon::install_daemon_service,
            commands::daemon::uninstall_daemon_service,
            commands::daemon::start_daemon_service,
            commands::daemon::stop_daemon_service,
            commands::daemon::is_daemon_service_installed,
            commands::daemon::is_daemon_service_running,
            commands::daemon::get_daemon_logs,
            commands::daemon::check_easytier_binary,
            commands::daemon::shutdown_daemon,
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

                // 1. 断开所有运行中的空间（停止服务、通知 daemon）
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
                            // 等待 daemon 完全关闭（它需要时间停止网络实例）
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        });
                    }
                }

                // 3. 强制终止 daemon 子进程（备降兜底）
                if let Some(guard) = app_handle.try_state::<DaemonGuard>() {
                    if let Ok(mut child_opt) = guard.0.lock() {
                        if let Some(mut child) = child_opt.take() {
                            let _ = child.kill();
                            let _ = child.wait();
                            crate::log_info!("[GUI] daemon 子进程已终止");
                        }
                    }
                }

                // 4. 最终清理（macOS root daemon + temp files）
                cleanup::shutdown_exit_cleanup();
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

/// 守护进程入口点（--daemon 模式）
pub fn run_daemon() -> std::process::ExitCode {
    // 创建 tokio 运行时
    let rt = tokio::runtime::Runtime::new()
        .expect("创建 tokio 运行时失败");

    // 运行守护进程
    let result = rt.block_on(daemon::run_daemon_async());

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
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn spawn_daemon() -> Result<std::process::Child, String> {
    use std::process::{Command, Stdio};

    let current_exe = std::env::current_exe().map_err(|e| format!("获取当前可执行文件路径失败: {}", e))?;

    crate::log_info!("[GUI] 启动 daemon 子进程");

    let mut cmd = Command::new(current_exe);
    cmd.arg("--daemon");
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.spawn().map_err(|e| {
        let msg = format!("启动 daemon 失败: {}", e);
        crate::log_error!(&msg);
        msg
    })
}