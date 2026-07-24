use std::path::PathBuf;

/// 服务管理器 trait
pub trait ServiceManager {
    /// 安装服务
    fn install(&self) -> Result<(), String>;
    /// 卸载服务
    fn uninstall(&self) -> Result<(), String>;
    /// 启动服务
    fn start(&self) -> Result<(), String>;
    /// 停止服务
    fn stop(&self) -> Result<(), String>;
    /// 检查服务是否已安装
    fn is_installed(&self) -> bool;
    /// 检查服务是否正在运行
    fn is_running(&self) -> bool;
}

/// 获取平台特定的服务管理器
pub fn get_service_manager() -> Box<dyn ServiceManager> {
    #[cfg(target_os = "linux")]
    return Box::new(SystemdServiceManager);
    #[cfg(target_os = "macos")]
    return Box::new(LaunchdServiceManager);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Box::new(NoopServiceManager);
}

/// 获取守护进程可执行文件路径
fn get_daemon_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))
}

/// 获取服务名称
fn get_service_name() -> &'static str {
    "hometier-daemon"
}

/// systemd 服务管理器（Linux）
#[cfg(target_os = "linux")]
struct SystemdServiceManager;

#[cfg(target_os = "linux")]
impl SystemdServiceManager {
    fn get_unit_file_path() -> PathBuf {
        PathBuf::from("/etc/systemd/system/hometier-daemon.service")
    }

    fn generate_unit_file() -> Result<String, String> {
        let exe_path = get_daemon_exe_path()?;
        let exe_str = exe_path.to_str().ok_or("路径包含非 UTF-8 字符")?;

        // 获取当前用户
        let user = std::env::var("USER").unwrap_or_else(|_| "root".into());

        Ok(format!(
            r#"[Unit]
Description=homeTier Daemon - Virtual LAN Service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
ExecStart={exe} --daemon
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
"#,
            user = user,
            exe = exe_str,
        ))
    }
}

#[cfg(target_os = "linux")]
impl ServiceManager for SystemdServiceManager {
    fn install(&self) -> Result<(), String> {
        let unit_content = Self::generate_unit_file()?;
        let unit_path = Self::get_unit_file_path();

        // 使用 pkexec 写入 unit 文件
        let status = std::process::Command::new("pkexec")
            .args(["tee", unit_path.to_str().unwrap_or("")])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动 pkexec 失败: {}", e))?
            .stdin
            .ok_or("无法获取 stdin")?;

        // 写入内容
        use std::io::Write;
        let mut stdin = status;
        stdin.write_all(unit_content.as_bytes())
            .map_err(|e| format!("写入 unit 文件失败: {}", e))?;
        drop(stdin);

        // 重新加载 systemd 配置
        std::process::Command::new("pkexec")
            .args(["systemctl", "daemon-reload"])
            .status()
            .map_err(|e| format!("重新加载 systemd 失败: {}", e))?;

        // 启用服务
        std::process::Command::new("pkexec")
            .args(["systemctl", "enable", get_service_name()])
            .status()
            .map_err(|e| format!("启用服务失败: {}", e))?;

        log_service("服务安装成功");
        Ok(())
    }

    fn uninstall(&self) -> Result<(), String> {
        // 停止服务
        let _ = self.stop();

        // 禁用服务
        std::process::Command::new("pkexec")
            .args(["systemctl", "disable", get_service_name()])
            .status()
            .map_err(|e| format!("禁用服务失败: {}", e))?;

        // 删除 unit 文件
        let unit_path = Self::get_unit_file_path();
        std::process::Command::new("pkexec")
            .args(["rm", unit_path.to_str().unwrap_or("")])
            .status()
            .map_err(|e| format!("删除 unit 文件失败: {}", e))?;

        // 重新加载 systemd 配置
        std::process::Command::new("pkexec")
            .args(["systemctl", "daemon-reload"])
            .status()
            .map_err(|e| format!("重新加载 systemd 失败: {}", e))?;

        log_service("服务卸载成功");
        Ok(())
    }

    fn start(&self) -> Result<(), String> {
        std::process::Command::new("pkexec")
            .args(["systemctl", "start", get_service_name()])
            .status()
            .map_err(|e| format!("启动服务失败: {}", e))?;
        log_service("服务已启动");
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        std::process::Command::new("pkexec")
            .args(["systemctl", "stop", get_service_name()])
            .status()
            .map_err(|e| format!("停止服务失败: {}", e))?;
        log_service("服务已停止");
        Ok(())
    }

    fn is_installed(&self) -> bool {
        Self::get_unit_file_path().exists()
    }

    fn is_running(&self) -> bool {
        std::process::Command::new("systemctl")
            .args(["is-active", get_service_name()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// launchd 服务管理器（macOS）
#[cfg(target_os = "macos")]
struct LaunchdServiceManager;

#[cfg(target_os = "macos")]
impl LaunchdServiceManager {
    fn get_plist_path() -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.home_dir().join("Library/LaunchAgents/com.hometier.daemon.plist"))
            .unwrap_or_else(|| PathBuf::from("/tmp/com.hometier.daemon.plist"))
    }

    fn generate_plist() -> Result<String, String> {
        let exe_path = get_daemon_exe_path()?;
        let exe_str = exe_path.to_str().ok_or("路径包含非 UTF-8 字符")?;

        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.hometier.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/hometier-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/hometier-daemon.err</string>
</dict>
</plist>
"#,
            exe = exe_str,
        ))
    }
}

#[cfg(target_os = "macos")]
impl ServiceManager for LaunchdServiceManager {
    fn install(&self) -> Result<(), String> {
        let plist_content = Self::generate_plist()?;
        let plist_path = Self::get_plist_path();

        // 写入 plist 文件
        std::fs::write(&plist_path, &plist_content)
            .map_err(|e| format!("写入 plist 文件失败: {}", e))?;

        // 加载服务
        std::process::Command::new("launchctl")
            .args(["load", plist_path.to_str().unwrap_or("")])
            .status()
            .map_err(|e| format!("加载服务失败: {}", e))?;

        log_service("服务安装成功");
        Ok(())
    }

    fn uninstall(&self) -> Result<(), String> {
        let plist_path = Self::get_plist_path();

        // 卸载服务
        std::process::Command::new("launchctl")
            .args(["unload", plist_path.to_str().unwrap_or("")])
            .status()
            .map_err(|e| format!("卸载服务失败: {}", e))?;

        // 删除 plist 文件
        std::fs::remove_file(&plist_path)
            .map_err(|e| format!("删除 plist 文件失败: {}", e))?;

        log_service("服务卸载成功");
        Ok(())
    }

    fn start(&self) -> Result<(), String> {
        let plist_path = Self::get_plist_path();
        std::process::Command::new("launchctl")
            .args(["start", "com.hometier.daemon"])
            .status()
            .map_err(|e| format!("启动服务失败: {}", e))?;
        log_service("服务已启动");
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        std::process::Command::new("launchctl")
            .args(["stop", "com.hometier.daemon"])
            .status()
            .map_err(|e| format!("停止服务失败: {}", e))?;
        log_service("服务已停止");
        Ok(())
    }

    fn is_installed(&self) -> bool {
        Self::get_plist_path().exists()
    }

    fn is_running(&self) -> bool {
        std::process::Command::new("launchctl")
            .args(["list", "com.hometier.daemon"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// 空操作服务管理器（不支持的平台）
struct NoopServiceManager;

impl ServiceManager for NoopServiceManager {
    fn install(&self) -> Result<(), String> {
        Err("此平台不支持自动安装服务".into())
    }

    fn uninstall(&self) -> Result<(), String> {
        Err("此平台不支持自动卸载服务".into())
    }

    fn start(&self) -> Result<(), String> {
        Err("此平台不支持自动启动服务".into())
    }

    fn stop(&self) -> Result<(), String> {
        Err("此平台不支持自动停止服务".into())
    }

    fn is_installed(&self) -> bool {
        false
    }

    fn is_running(&self) -> bool {
        false
    }
}

fn log_service(msg: &str) {
    crate::log_info!("[Service] {}", msg);
}
