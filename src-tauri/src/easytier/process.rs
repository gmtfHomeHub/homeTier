use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// EasyTier 子进程管理器
pub struct EasyTierProcess {
    child: Option<Child>,
    config_path: PathBuf,
    binary_path: PathBuf,
    /// RPC 端口（用于查询运行时状态）
    rpc_port: Option<u16>,
}

impl EasyTierProcess {
    /// 启动 easytier-core 子进程
    pub fn start(binary: &PathBuf, config: &PathBuf, rpc_port: Option<u16>) -> Result<Self, String> {
        let rpc_arg = rpc_port.unwrap_or(15888);
        crate::log_info!(format!("[EasyTierProcess] 启动: {} --config-file {} --rpc-portal {}", binary.display(), config.display(), rpc_arg));

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

        let child = cmd.spawn().map_err(|e| {
            let msg = format!("启动 easytier-core 失败: {}", e);
            crate::log_error!(&msg);
            msg
        })?;

        crate::log_info!(format!("[EasyTierProcess] 进程已启动, pid={}, rpc_port={}", child.id(), rpc_arg));
        Ok(Self { child: Some(child), config_path: config.clone(), binary_path: binary.clone(), rpc_port: Some(rpc_arg) })
    }

    /// 获取进程 ID
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// 获取 RPC 端口
    pub fn rpc_port(&self) -> Option<u16> {
        self.rpc_port
    }

    /// 检查进程是否正在运行
    pub fn is_running(&self) -> bool {
        if let Some(ref child) = self.child {
            // try_wait 检查子进程状态，不阻塞
            match child.try_wait() {
                Ok(Some(_)) => false, // 进程已退出
                Ok(None) => true,     // 进程仍在运行
                Err(_) => false,      // 错误，假设已退出
            }
        } else {
            false
        }
    }

    /// 停止进程
    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(ref mut child) = self.child {
            crate::log_info!(format!("[EasyTierProcess] 停止进程, pid={}", child.id()));
            child.kill().map_err(|e| format!("终止进程失败: {}", e))?;
            child.wait().map_err(|e| format!("等待进程退出失败: {}", e))?;
            self.child = None;
            crate::log_info!("[EasyTierProcess] 进程已停止");
        }
        Ok(())
    }

    /// 重启进程
    pub fn restart(&mut self, new_config: Option<&PathBuf>) -> Result<(), String> {
        self.stop()?;
        let config = new_config.unwrap_or(&self.config_path);
        let mut new_child = Command::new(&self.binary_path)
            .arg("--config-file")
            .arg(config)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("重启 easytier-core 失败: {}", e))?;

        crate::log_info!(format!("[EasyTierProcess] 进程已重启, pid={}", new_child.id()));
        self.child = Some(new_child);
        self.config_path = config.clone();
        Ok(())
    }

    /// 健康检查：尝试连接 TCP RPC 端口
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
        if self.child.is_some() {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_not_running() {
        // 未启动的进程应该返回 false
        let config = PathBuf::from("/tmp/test.toml");
        let binary = PathBuf::from("/tmp/test_binary");
        let proc = EasyTierProcess { child: None, config_path: config, binary_path: binary };
        assert!(!proc.is_running());
    }
}
