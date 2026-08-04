pub mod app;
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
pub mod server;
pub mod space;
pub mod types;
pub mod voice;

use std::sync::Arc;

use tauri::Manager;


// 全局 daemon 子进程引用（供 Exit 事件兜底 kill 使用）

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

    let builder = crate::proxy::hometier_protocol::register_protocol(builder);

    let builder = builder.setup(|app| crate::app::setup::setup(app))
        .invoke_handler(tauri::generate_handler![
            commands::space::create_space,
            commands::space::join_space,
            commands::space::leave_space,
            commands::space::delete_space,
            commands::space::list_spaces,
            commands::space::list_members,
            commands::space::get_space_config,
            commands::space::update_space_config,
            commands::space::generate_share_link,
            commands::space::parse_share_link,
            commands::space::connect_space,
            commands::space::disconnect_space,
            commands::space::get_space_status,
            commands::space::patch_space_config,
            commands::network::get_network_stats,
            commands::network::update_group_config,
            commands::network::get_space_peers,
            commands::chat::send_message,
            commands::chat::get_message_history,
            commands::signal::send_signal,
            commands::voice::join_voice_channel,
            commands::voice::leave_voice_channel,
            commands::voice::toggle_mic,
            commands::voice::toggle_speaker,
            commands::file::send_file,
            commands::file::receive_file,
            commands::file::list_files,
            commands::file::get_transfer_progress,
            commands::file::record_received_file,
            commands::file::delete_file,
            commands::screen::start_screen_share,
            commands::screen::stop_screen_share,
            commands::screen::is_screen_sharing,
            commands::screen::get_screen_share_viewers,
            commands::util::get_app_version,
            commands::util::get_system_config,
            commands::util::set_system_config,
            commands::util::get_log_enabled,
            commands::util::set_log_enabled,
            commands::log::get_logs,
            commands::log::get_space_logs,
            commands::log::clear_logs,
            commands::app::add_app,
            commands::app::update_app,
            commands::app::delete_app,
            commands::app::list_apps,
            commands::config::get_app_config,
            commands::config::set_app_config,
            commands::config::get_config_file_path,
            commands::config::get_config_template_path,
            commands::proxy::get_proxy_url,
            commands::proxy::get_proxy_status,
            commands::proxy::register_proxy_key,
            commands::proxy::set_proxy_source,
            commands::tray::update_tray_menu,
            commands::daemon::is_daemon_ready,
            commands::daemon::get_daemon_error_reason,
            commands::daemon::get_daemon_logs,
            commands::daemon::check_easytier_binary,
            commands::easytier::get_easytier_version,
            commands::easytier::check_easytier_update,
            commands::easytier::upgrade_easytier,
            commands::easytier::upgrade_easytier_with_progress,
            commands::network_acls::get_acl_rules,
            commands::network_acls::create_acl_rule,
            commands::network_acls::update_acl_rule,
            commands::network_acls::delete_acl_rule,
            commands::network_port_forwards::get_port_forward_rules,
            commands::network_port_forwards::create_port_forward_rule,
            commands::network_port_forwards::update_port_forward_rule,
            commands::network_port_forwards::delete_port_forward_rule,
        ])
        .on_window_event(|_win, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = _win.hide();
                #[cfg(target_os = "macos")]
                {
                    use tauri::ActivationPolicy;
                    let _ = _win.app_handle().set_activation_policy(ActivationPolicy::Accessory);
                }
                api.prevent_close();
            }
        });

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        #[cfg(target_os = "macos")]
        crate::app::window::activate_main_window(app);
        #[cfg(not(target_os = "macos"))]
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(e) => {
            crate::log_error!(format!("应用构建失败: {}", e));
            return std::process::ExitCode::FAILURE;
        }
    };

    app.run(|app_handle, event| {
        match event {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            tauri::RunEvent::Exit => {
                crate::app::exit::on_exit_cleanup(app_handle);
            }
            _ => {}
        }
    });

    // 应用退出时关闭代理服务
    if let Some(_proxy) = crate::app::PROXY_SERVER.get() {
        crate::log_info!("应用退出，代理服务器自动关闭");
    }

    // 应用退出时清空日志
    crate::log::clear();

    std::process::ExitCode::SUCCESS
}

/// 带参数的入口点，用于 Windows UAC / macOS 提权场景
pub fn run_with_args(_elevated: bool) -> std::process::ExitCode {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        crate::app::window::set_elevated(elevated);
        if elevated {
            crate::log_info!("提权进程已启动");
        }
    }
    run()
}

/// 守护进程入口点（--daemon 模式，路径从 CLI 参数传入）
pub fn run_daemon(
    config_dir: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    gui_pid: Option<u32>,
    resource_dir: Option<std::path::PathBuf>,
) -> std::process::ExitCode {
    crate::config::init(data_dir.join("homeTier.conf"), resource_dir.as_deref());
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio 运行时失败");
    let result = rt.block_on(daemon::run_daemon_async(config_dir, data_dir, gui_pid, resource_dir));
    match result {
        Ok(()) => {
            crate::log_info!("守护进程正常退出");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            crate::log_error!("守护进程异常退出: {}", e);
            std::process::ExitCode::FAILURE
        }
    }
}

/// 服务器模式入口点（--server 模式，Web 管理界面 + REST API + 静态文件服务）
pub fn run_server(
    bind: Option<String>,
    port: Option<u16>,
    server_dir: Option<std::path::PathBuf>,
    resource_dir: Option<std::path::PathBuf>,
    static_dir_override: Option<std::path::PathBuf>,
) -> std::process::ExitCode {
    let data_dir = server_dir.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_default().join("homeTier-data")
    });
    let config_path = data_dir.join("homeTier.conf");
    crate::config::init(config_path, resource_dir.as_deref());

    crate::log::init_logger(
        Some("server"),
        Some(&data_dir),
        None,
    );

    let server_config = crate::server::init_server_config(&data_dir);
    let auth_state = crate::server::auth::init_auth_secret(&server_config);

    let db_path = data_dir.join("homeTier.db");
    let db = Arc::new(crate::db::Database::new(&db_path).expect("数据库初始化失败"));

    let easytier_config_dir = data_dir.join("easytier");
    let easy_tier = Arc::new(crate::easytier::EasyTierManager::new(
        easytier_config_dir.clone(),
        data_dir.clone(),
        resource_dir.as_deref(),
    ));

    let event_bus = Arc::new(crate::server::event::GlobalEventBus::new(100));

    // 文件传输：与桌面模式一致的 P2P 管理器 + 本地文件服务器注册表（存储目录 {data_dir}/files）
    let file_manager = Arc::new(crate::file::FileTransferManager::new());
    let file_registry = Arc::new(crate::file::FileServerRegistry::new(
        data_dir.join("files"),
        Arc::clone(&db),
    ));

    // 启动内部 HTTP 代理（与桌面模式一致：绕过 iframe 安全限制 + 反向代理）
    let proxy_server = crate::server::init_proxy_server();

    let rt = tokio::runtime::Runtime::new().expect("创建 tokio 运行时失败");
    let result = rt.block_on(async {
        // 服务器模式复用 daemon 基础设施：内嵌启动 daemon（IPC + easytier-core 管理），
        // SpaceManager 通过 IpcClient 与之通信，与桌面模式完全一致
        let daemon_config_dir = easytier_config_dir.clone();
        let daemon_data_dir = data_dir.clone();
        let daemon_resource_dir = resource_dir.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::daemon::run_daemon_async(
                daemon_config_dir,
                daemon_data_dir,
                None,
                daemon_resource_dir,
            )
            .await
            {
                crate::log_error!("内嵌 daemon 异常退出: {}", e);
            }
        });

        // 等待 daemon IPC 就绪后创建空间管理器
        let ipc_client = Arc::new(crate::daemon::client::IpcClient::default_port());
        let mut ready = false;
        for _ in 0..100 {
            if ipc_client.ping().await {
                ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if !ready {
            return Err("等待 daemon IPC 就绪超时".to_string());
        }
        crate::log_info!("[Server] daemon IPC 已就绪");

        let space_manager = Arc::new(crate::space::manager::SpaceManager::new(
            Arc::clone(&db),
            Arc::clone(&easy_tier),
            ipc_client,
        ));

        let app_state = Arc::new(crate::server::AppState {
            db,
            space_manager,
            easy_tier,
            config: server_config,
            auth_secret: auth_state.secret,
            event_bus,
            proxy_server,
            file_manager,
            file_registry,
        });

        crate::server::start_server(
            &bind.unwrap_or_else(|| app_state.config.get_str("SERVER_BIND", "0.0.0.0")),
            port.unwrap_or_else(|| app_state.config.get_u16("SERVER_PORT", 9339)),
            static_dir_override
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| app_state.config.get_str("SERVER_STATIC_DIR", "./dist")),
            app_state,
        )
        .await
    });

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            crate::log_error!("服务器异常退出: {}", e);
            std::process::ExitCode::FAILURE
        }
    }
}