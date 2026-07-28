use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tokio::process::{Child, Command};

/// EasyTier 子进程管理器
pub struct EasyTierProcess {
    #[cfg(not(target_os = "macos"))]
    child: Mutex<Option<Child>>,
    #[cfg(target_os = "macos")]
    child: Mutex<Option<u32>>,
    config_path: PathBuf,
    binary_path: PathBuf,
    /// RPC 端口（用于查询运行时状态）
    rpc_port: Option<u16>,
}

impl EasyTierProcess {
    /// 启动 easytier-core 子进程
    #[cfg(not(target_os = "macos"))]
    pub async fn start(binary: &PathBuf, config: &PathBuf, rpc_port: Option<u16>) -> Result<Self, String> {
        let rpc_arg = rpc_port.unwrap_or(15888);
        crate::log_info!(format!("[EasyTierProcess] 启动: {} → --config-file {} --rpc-portal {}", binary.display(), config.display(), rpc_arg));

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
        Ok(Self { child: Mutex::new(Some(child)), config_path: config.clone(), binary_path: binary.clone(), rpc_port: Some(rpc_arg) })
    }

    /// macOS: 通过 osascript 以 root 权限启动 easytier-core
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

        crate::log_info!(format!("[EasyTierProcess] macOS osascript 已触发, rpc_port={}", rpc_arg));
        Ok(Self { child: Mutex::new(Some(0)), config_path: config.clone(), binary_path: binary.clone(), rpc_port: Some(rpc_arg) })
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
                let _ = stream.shutdown().await;
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
        let config = new_config.unwrap_or(&self.config_path);

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
            self.config_path = config.clone();
            return Ok(());
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
            self.config_path = config.clone();
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
        let config = PathBuf::from("/tmp/test.toml");
        let binary = PathBuf::from("/tmp/test_binary");
        let proc = EasyTierProcess {
            child: Mutex::new(None),
            config_path: config,
            binary_path: binary,
            rpc_port: None,
        };
        assert!(!proc.is_running());
    }
}
