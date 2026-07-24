pub mod commands;
pub mod chat;
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
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tauri::Manager;

use commands::app_view::AppWebview;
use proxy::plugins::*;
use proxy::{ActiveOrigin, ProxyHandler, ProxyKeyMap};

/// 全局代理服务器，用于应用退出时关闭
static PROXY_SERVER: OnceLock<Arc<proxy::ProxyServer>> = OnceLock::new();

/// Windows UAC 提权标记，用于检测当前进程是否通过 runas 启动
#[cfg(target_os = "windows")]
static ELEVATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 检查当前进程是否以提权模式运行（Windows UAC）
#[cfg(target_os = "windows")]
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
                db::Database::new(&db_path).expect("Failed to initialize database"),
            );
            app.manage(db.clone());

            // 初始化 EasyTier 实例管理器
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let easytier_config_dir = app_data.join("easytier");
            let _ = std::fs::create_dir_all(&easytier_config_dir);
            let instance_manager = Arc::new(easytier::EasyTierManager::new(easytier_config_dir));
            app.manage(instance_manager.clone());

            // 初始化空间管理器
            let space_manager = Arc::new(space::manager::SpaceManager::new(
                db,
                instance_manager,
            ));
            app.manage(space_manager);

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
                Arc::new(WebSocketPlugin),
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
            // 网络管理
            commands::network::get_network_status,
            commands::network::get_network_stats,
            commands::network::update_group_config,
            commands::network::update_local_config,
            commands::network::get_effective_config,
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
            commands::daemon::daemon_connect_space,
            commands::daemon::daemon_disconnect_space,
            commands::daemon::daemon_list_spaces,
            commands::daemon::install_daemon_service,
            commands::daemon::uninstall_daemon_service,
            commands::daemon::start_daemon_service,
            commands::daemon::stop_daemon_service,
            commands::daemon::is_daemon_service_installed,
            commands::daemon::is_daemon_service_running,
            commands::daemon::shutdown_daemon,
        ])
        .on_window_event(|_win, event| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
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

    builder
        .run(tauri::generate_context!())
        .expect("error while running homeTier");

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

/// 带参数的入口点，用于 Windows UAC 提权场景
pub fn run_with_args(elevated: bool) -> std::process::ExitCode {
    #[cfg(target_os = "windows")]
    {
        ELEVATED.store(elevated, std::sync::atomic::Ordering::SeqCst);
        if elevated {
            log_info!("Windows UAC 提权进程已启动");
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