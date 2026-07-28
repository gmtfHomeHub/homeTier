use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tokio::process::{Child, Command};

/// EasyTier core 进程包装
///
/// macOS: 应用启动时通过 osascript 提权启动守护进程（idle, 无网络配置），
///        后续通过 RPC run_network_instance / delete_network_instance 热切换网络，
///        无需再次弹窗授权。
/// 其他平台：直接作为子进程管理。
pub struct EasyTierProcess {
    #[cfg(not(target_os = "macos"))]
    child: Mutex<Option<Child>>,
    #[cfg(target_os = "macos")]
    child: Mutex<Option<u32>>,
    binary_path: PathBuf,
    config_dir: PathBuf,
    /// RPC 端口
    rpc_port: Option<u16>,
}

impl EasyTierProcess {
    #[cfg(not(target_os = "macos"))]
    pub async fn start(binary: &PathBuf, config: &PathBuf, rpc_port: Option<u16>) -> Result<Self, String> {
        let rpc_arg = rpc_port.unwrap_or(15888);
        crate::log_info!(format!("[EasyTierProcess] 启动: {} --config-file {} --rpc-portal {}", binary.display(), config.display(), rpc_arg));

        let config_dir = config.parent().map(|p| p.to_path_buf()).unwrap_or_default();

        let mut cmd = Command::new(binary);
        cmd.arg("--config-file").arg(config);
        cmd.arg("--rpc-portal").arg(rpc_arg.to_string());
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("启动 easytier-core 失败: {}", e);
            crate::log_error!(&msg);
            msg
        })?;

        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let reader = tokio::io::BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    crate::log_info!(format!("[easytier stdout] {}", line));
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let reader = tokio::io::BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    crate::log_info!(format!("[easytier stderr] {}", line));
                }
            });
        }

        crate::log_info!(format!("[EasyTierProcess] 进程已启动, pid={:?}, rpc_port={}", child.id(), rpc_arg));
        Ok(Self { child: Mutex::new(Some(child)), binary_path: binary.clone(), config_dir, rpc_port: Some(rpc_arg) })
    }

    /// macOS: 通过 osascript 以 root 权限启动 easytier-core 守护进程（idle 模式）
    /// 先写入临时脚本到 /tmp/easytier-daemon.sh，再通过 osascript 执行该脚本。
    /// 这样做的好处：避免 osascript 的复杂引号转义；可以用 nohup 保持进程；
    /// easytier-core 的输出记录到日志文件，方便调试。
    #[cfg(target_os = "macos")]
    pub async fn start_daemon(binary: &PathBuf, config_dir: &PathBuf, rpc_port: u16) -> Result<Self, String> {
        let _ = std::fs::create_dir_all(config_dir);
        crate::log_info!(format!("[EasyTierProcess] macOS 守护进程启动, config_dir={}", config_dir.display()));
        crate::log_info!(format!("[EasyTierProcess] 二进制: {}, RPC端口: {}", binary.display(), rpc_port));

        let log_file = config_dir.join("easytier-daemon.log");
        let script_path = std::path::PathBuf::from("/tmp/easytier-daemon-launch.sh");
        let script_content = format!(
            r#"#!/bin/sh
nohup "{}" --rpc-portal {} --daemon --config-dir "{}" --log-file "{}" > "{}" 2>&1 &
EASETIERD_PID=$!
echo "easytier-core pid=$EASETIERD_PID"
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    nc -z 127.0.0.1 {} > /dev/null 2>&1 && exit 0
    sleep 1
done
echo "ERROR: easytier-core RPC port not ready after 20s" >&2
echo "=== easytier-core stdout/stderr dump ===" >&2
cat "{}" >&2
exit 1
"#,
            binary.display(),
            rpc_port,
            config_dir.display(),
            log_file.display(),
            log_file.display(),
            rpc_port,
            log_file.display()
        );

        std::fs::write(&script_path, &script_content)
            .map_err(|e| format!("写入启动脚本失败: {}", e))?;

        crate::log_info!(format!("[EasyTierProcess] 启动脚本已写入: {}", script_path.display()));

        let escaped_script = script_path.as_path().to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let osascript_program = format!(
            "do shell script \"/bin/sh \"{}\"\" with administrator privileges",
            escaped_script
        );

        crate::log_info!(format!("[EasyTierProcess] 正在弹出授权对话框..."));

        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&osascript_program)
            .output()
            .map_err(|e| {
                let msg = format!("macOS 提权启动守护进程失败: {}", e);
                crate::log_error!(&msg);
                msg
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        crate::log_info!(format!("[EasyTierProcess] osascript stdout: {}", stdout.trim()));
        if !stderr_str.is_empty() {
            if stderr_str.contains("User canceled") || stderr_str.contains("canceled") {
                return Err("用户取消了授权".to_string());
            }
            crate::log_error!(format!("[EasyTierProcess] osascript stderr: {}", stderr_str));
            crate::log_error!(format!("[EasyTierProcess] easytier-daemon.log 内容: {}",
                std::fs::read_to_string(&log_file).unwrap_or_else(|_| "(无法读取)".into())
            ));
            return Err(format!("守护进程启动脚本失败: {}", stderr_str));
        }

        if !output.status.success() {
            let log_content = std::fs::read_to_string(&log_file).unwrap_or_else(|_| "(无法读取)".into());
            crate::log_error!(format!("[EasyTierProcess] launchd 日志: {}", log_content));
            return Err(format!("启动脚本退出码: {}", output.status));
        }

        crate::log_info!(format!("[EasyTierProcess] macOS osascript 授权成功, easytier-core 应在端口 {} 监听", rpc_port));
        Ok(Self { child: Mutex::new(Some(0)), binary_path: binary.clone(), config_dir: config_dir.clone(), rpc_port: Some(rpc_port) })
    }

    /// macOS: 通过 osascript 以 root 权限启动 easytier-core（带配置文件的单网络模式，保留兼容）
    #[cfg(target_os = "macos")]
    pub async fn start(binary: &PathBuf, config: &PathBuf, rpc_port: Option<u16>) -> Result<Self, String> {
        let rpc_arg = rpc_port.unwrap_or(15888);
        crate::log_info!(format!("[EasyTierProcess] macOS 提权启动: {} --config-file {} --rpc-portal {}", binary.display(), config.display(), rpc_arg));

        let escaped_binary = binary.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_config = config.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "do shell script \"{} --config-file '{}' --rpc-portal {} --no-log-file > /dev/null 2>&1 &\" with administrator privileges",
            escaped_binary, escaped_config, rpc_arg
        );

        std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let msg = format!("macOS 提权启动 easytier-core 失败: {}", e);
                crate::log_error!(&msg);
                msg
            })?;

        let config_dir = config.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        crate::log_info!(format!("[EasyTierProcess] macOS osascript 已触发, rpc_port={}", rpc_arg));
        Ok(Self { child: Mutex::new(Some(0)), binary_path: binary.clone(), config_dir, rpc_port: Some(rpc_arg) })
    }

    /// 获取进程 ID
    #[cfg(not(target_os = "macos"))]
    pub fn pid(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|guard| guard.as_ref().and_then(|c| c.id()))
    }

    #[cfg(target_os = "macos")]
    pub fn pid(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|guard| *guard)
    }

    /// 获取 RPC 端口
    pub fn rpc_port(&self) -> Option<u16> {
        self.rpc_port
    }

    /// 检查进程是否正在运行（通过 RPC 端口可连性判断）
    pub fn is_running(&self) -> bool {
        match self.rpc_port {
            Some(port) => {
                let addr = match format!("127.0.0.1:{}", port).parse() {
                    Ok(a) => a,
                    Err(_) => return false,
                };
                match std::net::TcpStream::connect_timeout(
                    &addr,
                    std::time::Duration::from_millis(200),
                ) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
            None => false,
        }
    }

    /// 停止进程（macOS 通过 RPC shutdown 端点；其他平台 kill 子进程）
    #[cfg(not(target_os = "macos"))]
    pub async fn stop(&self) -> Result<(), String> {
        let mut child_opt = self.child.lock().map_err(|e| format!("锁获取失败: {}", e))?.take();
        if let Some(ref mut child) = child_opt {
            crate::log_info!(format!("[EasyTierProcess] 停止进程, pid={:?}", child.id()));
            child.kill().await.map_err(|e| format!("终止进程失败: {}", e))?;
            child.wait().await.map_err(|e| format!("等待进程退出失败: {}", e))?;
            crate::log_info!("[EasyTierProcess] 进程已停止");
        }
        *self.child.lock().map_err(|e| format!("锁获取失败: {}", e))? = child_opt;
        Ok(())
    }

    /// macOS: 通过 RPC 发送 shutdown 请求停止 easytier-core 进程
    #[cfg(target_os = "macos")]
    pub async fn stop(&self) -> Result<(), String> {
        let port = self.rpc_port.ok_or("RPC 端口未知".to_string())?;
        let addr = format!("127.0.0.1:{}", port);
        crate::log_info!(format!("[EasyTierProcess] 通过 RPC shutdown 停止进程, port={}", port));
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(stream) => {
                use tokio::io::AsyncWriteExt;
                let _ = stream.writable().await;
                let _ = stream.try_write(b"__RPC_SHUTDOWN__\n");
            }
            Err(e) => {
                crate::log_warn!(format!("[EasyTierProcess] RPC 端口不可达, 无法发送 shutdown: {}", e));
            }
        }
        *self.child.lock().map_err(|e| format!("锁获取失败: {}", e))? = None;
        Ok(())
    }

    /// 重启进程
    pub async fn restart(&mut self, new_config: Option<&PathBuf>) -> Result<(), String> {
        self.stop().await?;
        let config = new_config.unwrap_or(&self.config_dir);

        #[cfg(target_os = "macos")]
        {
            let rpc_port = self.rpc_port.unwrap_or(15888);
            let escaped_binary = self.binary_path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
            let escaped_config = config.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                "do shell script \"{} --config-file '{}' --rpc-portal {} --no-log-file > /dev/null 2>&1 &\" with administrator privileges",
                escaped_binary, escaped_config, rpc_port
            );
            std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("重启 easytier-core 失败: {}", e))?;
            *self.child.lock().map_err(|e| format!("锁获取失败: {}", e))? = Some(0);
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let new_child = Command::new(&self.binary_path)
                .arg("--config-file")
                .arg(config)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("重启 easytier-core 失败: {}", e))?;

            crate::log_info!(format!("[EasyTierProcess] 进程已重启, pid={:?}", new_child.id()));
            *self.child.lock().map_err(|e| format!("锁获取失败: {}", e))? = Some(new_child);
            Ok(())
        }
    }

    pub async fn check_health(&self, rpc_port: u16) -> bool {
        let addr = format!("127.0.0.1:{}", rpc_port);
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

impl Drop for EasyTierProcess {
    fn drop(&mut self) {
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(mut guard) = self.child.lock() {
                if let Some(ref mut child) = *guard {
                    let _ = child.kill();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_not_running() {
        let config_dir = PathBuf::from("/tmp");
        let binary = PathBuf::from("/tmp/test_binary");
        let proc = EasyTierProcess {
            child: Mutex::new(None),
            binary_path: binary,
            config_dir,
            rpc_port: None,
        };
        assert!(!proc.is_running());
    }
}
