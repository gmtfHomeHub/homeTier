use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tokio::process::{Child, Command};

/// EasyTier core 进程包装
///
/// daemon 以 root 运行（macOS 经 osascript 提权启动 daemon），
/// easytier-core 作为 daemon 的直接子进程启动，继承 root 权限，
/// 因此 daemon 可通过 child.kill() 直接终止它（root→root，无 EPERM）。
pub struct EasyTierProcess {
    child: Mutex<Option<Child>>,
    binary_path: PathBuf,
    config_dir: PathBuf,
    /// RPC 端口
    rpc_port: Option<u16>,
}

impl EasyTierProcess {
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

    /// 获取进程 ID
    pub async fn start_daemon(binary: &PathBuf, config_dir: &PathBuf, rpc_port: u16) -> Result<Self, String> {
        let _ = std::fs::create_dir_all(config_dir);
        crate::log_info!(format!("[EasyTierProcess] 启动守护进程: {} --daemon --config-dir {} --rpc-portal {}", binary.display(), config_dir.display(), rpc_port));

        let mut cmd = Command::new(binary);
        cmd.arg("--daemon")
            .arg("--config-dir")
            .arg(config_dir)
            .arg("--rpc-portal")
            .arg(rpc_port.to_string());
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("启动 easytier-core 守护进程失败: {}", e);
            crate::log_error!(&msg);
            msg
        })?;

        // 捕获 stdout 到 LOG_STORE
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
        // 捕获 stderr 到 LOG_STORE
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

        let pid = child.id();
        crate::log_info!(format!("[EasyTierProcess] 守护进程已启动, pid={:?}, rpc_port={}", pid, rpc_port));

        // 将 PID 写入 config_dir，供退出清理兜底使用
        if let Some(pid) = pid {
            let pid_file = config_dir.join("easytier-core.pid");
            let _ = std::fs::write(&pid_file, pid.to_string());
        }

        // 等待 RPC 端口就绪（最多 20s）
        for i in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", rpc_port)).await {
                Ok(_) => {
                    crate::log_info!(format!("[EasyTierProcess] 守护进程 RPC 端口就绪 (尝试次数: {})", i + 1));
                    return Ok(Self {
                        child: Mutex::new(Some(child)),
                        binary_path: binary.clone(),
                        config_dir: config_dir.clone(),
                        rpc_port: Some(rpc_port),
                    });
                }
                Err(_) => {
                    if i % 10 == 9 {
                        crate::log_info!(format!("[EasyTierProcess] 等待 RPC 端口就绪 ({}/40)...", i + 1));
                    }
                }
            }
        }

        crate::log_error!(format!("[EasyTierProcess] 守护进程 RPC 端口未能在 20s 内就绪"));
        let _ = child.kill();
        Err("easytier-core 守护进程启动超时".to_string())
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|guard| guard.as_ref().and_then(|c| c.id()))
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

    /// 停止进程（终止并等待子进程退出；退出后移除 PID 文件，避免 GUI 兜底清理误报）
    pub async fn stop(&self) -> Result<(), String> {
        let mut child_opt = self.child.lock().map_err(|e| format!("锁获取失败: {}", e))?.take();
        if let Some(ref mut child) = child_opt {
            crate::log_info!(format!("[EasyTierProcess] 停止进程, pid={:?}", child.id()));
            child.kill().await.map_err(|e| format!("终止进程失败: {}", e))?;
            child.wait().await.map_err(|e| format!("等待进程退出失败: {}", e))?;
            crate::log_info!("[EasyTierProcess] 进程已停止");
            let pid_file = self.config_dir.join("easytier-core.pid");
            let _ = std::fs::remove_file(&pid_file);
        }
        *self.child.lock().map_err(|e| format!("锁获取失败: {}", e))? = child_opt;
        Ok(())
    }

    /// 重启进程
    pub async fn restart(&mut self, new_config: Option<&PathBuf>) -> Result<(), String> {
        self.stop().await?;
        let config = new_config.unwrap_or(&self.config_dir);

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
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                let _ = child.kill();
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
