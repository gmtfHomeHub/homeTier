use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

pub mod template;

/// 应用配置文件（.env 风格 KEY=VALUE）
///
/// 三层读取模式：
/// 模板层(homeTier.conf.example) > 运行时配置 > 内置默认值。
/// 通过文件 mtime 轮询实现热更新，修改后无需重启即生效。
pub struct AppConfig {
    path: PathBuf,
    inner: RwLock<HashMap<String, String>>,
    last_modified: RwLock<Option<SystemTime>>,
    /// 模板原文（save 时作为骨架，保留注释）
    template: RwLock<Option<String>>,
    /// 模板解析后的键值（get 回退）
    template_map: RwLock<HashMap<String, String>>,
    /// 模板来源路径（用于前端展示）
    template_path: RwLock<Option<PathBuf>>,
}

/// 全局配置实例（setup 阶段初始化）
static GLOBAL: OnceLock<AppConfig> = OnceLock::new();

/// 配置键（与注释模板一致）
pub const KEY_DAEMON_IPC_PORT: &str = "DAEMON_IPC_PORT";
pub const KEY_EASYTIER_RPC_PORT: &str = "EASYTIER_RPC_PORT";
pub const KEY_FILE_SERVER_PORT_BASE: &str = "FILE_SERVER_PORT_BASE";
pub const KEY_DEFAULT_SPACE_IP: &str = "DEFAULT_SPACE_IP";
pub const KEY_GITHUB_API: &str = "GITHUB_API";
pub const KEY_GITHUB_MIRROR: &str = "GITHUB_MIRROR";
pub const KEY_LOG_ENABLED: &str = "LOG_ENABLED";

/// 默认值
pub const DEFAULT_FILE_SERVER_PORT_BASE: u16 = 19000;
pub const DEFAULT_SPACE_IP: &str = "10.144.144.10";
pub const DEFAULT_GITHUB_API: &str = "https://api.github.com/repos/EasyTier/EasyTier/releases";
pub const DEFAULT_GITHUB_MIRROR: &str = "https://ghproxy.top";
pub const DEFAULT_LOG_ENABLED: bool = true;

impl AppConfig {
    pub fn new(path: PathBuf, template: Option<String>, template_path: Option<PathBuf>) -> Self {
        let template_map = template
            .as_deref()
            .map(parse_env)
            .unwrap_or_default();
        Self {
            path,
            inner: RwLock::new(HashMap::new()),
            last_modified: RwLock::new(None),
            template: RwLock::new(template),
            template_map: RwLock::new(template_map),
            template_path: RwLock::new(template_path),
        }
    }

    /// 从文件加载（文件不存在则忽略）
    pub fn load(&self) {
        if let Ok(content) = fs::read_to_string(&self.path) {
            let parsed = parse_env(&content);
            if let Ok(mut map) = self.inner.write() {
                *map = parsed;
            }
            if let Ok(meta) = fs::metadata(&self.path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(mut lm) = self.last_modified.write() {
                        *lm = Some(modified);
                    }
                }
            }
        }
    }

    /// 重新加载（mtime 变化时调用）
    pub fn reload(&self) {
        self.load();
    }

    /// 当前配置是否已发生外部变化（供轮询检测）
    pub fn has_external_change(&self) -> bool {
        let meta = match fs::metadata(&self.path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let modified = match meta.modified() {
            Ok(m) => m,
            Err(_) => return false,
        };
        let last = self.last_modified.read().map(|lm| *lm).unwrap_or(None);
        match last {
            Some(l) => modified != l,
            None => true,
        }
    }

    /// 读取配置值（优先级：运行时配置 > 模板默认值）
    pub fn get(&self, key: &str) -> Option<String> {
        let from_inner = self
            .inner
            .read()
            .map(|m| m.get(key).cloned())
            .unwrap_or(None);
        if from_inner.is_some() {
            return from_inner;
        }
        self.template_map
            .read()
            .map(|m| m.get(key).cloned())
            .unwrap_or(None)
    }

    /// 读取 u16 端口类配置
    pub fn get_u16(&self, key: &str, default: u16) -> u16 {
        self.get(key)
            .and_then(|v| v.trim().parse::<u16>().ok())
            .unwrap_or(default)
    }

    /// 读取布尔配置（明确 true 集 => true；明确 false 集 => false；无效值回退默认）
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        let raw = match self.get(key) {
            Some(v) => v.trim().to_lowercase(),
            None => return default,
        };
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        }
    }

    /// 读取字符串配置
    pub fn get_str(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    /// 写入配置（内存 + 落盘）
    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        if let Ok(mut map) = self.inner.write() {
            map.insert(key.to_string(), value.to_string());
        }
        self.save()
    }

    /// 将内存配置写回文件（以模板为骨架，保留注释；模板缺失时用内置模板）
    pub fn save(&self) -> Result<(), String> {
        let map = self.inner.read().map(|m| m.clone()).unwrap_or_default();
        let content = match self.template.read().map(|t| t.clone()).unwrap_or(None) {
            Some(raw) => render_from_template(&raw, &map),
            None => build_template(&map),
        };
        fs::write(&self.path, content).map_err(|e| format!("写入配置文件失败: {}", e))?;
        if let Ok(meta) = fs::metadata(&self.path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(mut lm) = self.last_modified.write() {
                    *lm = Some(modified);
                }
            }
        }
        Ok(())
    }

    /// 全部配置（合并模板默认值 + 运行时覆盖）
    pub fn all(&self) -> HashMap<String, String> {
        let mut result = self
            .template_map
            .read()
            .map(|m| m.clone())
            .unwrap_or_default();
        for (k, v) in self.inner.read().map(|m| m.clone()).unwrap_or_default() {
            result.insert(k, v);
        }
        result
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// 模板来源路径
    pub fn template_path(&self) -> Option<PathBuf> {
        self.template_path.read().map(|p| p.clone()).unwrap_or(None)
    }
}

/// 初始化全局配置（setup 阶段调用）。返回是否首次创建配置文件。
///
/// resource_dir 用于定位模板（生产打包资源目录）；daemon 等无 AppHandle 场景传 None。
pub fn init(path: PathBuf, resource_dir: Option<&Path>) -> bool {
    if GLOBAL.get().is_some() {
        return false;
    }
    let template_path = template::locate_template(resource_dir);
    let template = template_path
        .as_ref()
        .and_then(|p| template::read_template(p));
    let created = !path.exists();
    let config = AppConfig::new(path, template, template_path);
    if created {
        let _ = config.save();
    }
    config.load();
    let _ = GLOBAL.set(config);
    created
}

/// 全局配置引用（未初始化时返回 None）
pub fn global() -> Option<&'static AppConfig> {
    GLOBAL.get()
}

/// 读取字符串配置（回退默认值）
pub fn get_str(key: &str, default: &str) -> String {
    GLOBAL
        .get()
        .map(|c| c.get_str(key, default))
        .unwrap_or_else(|| default.to_string())
}

/// 读取 u16 配置（回退默认值）
pub fn get_u16(key: &str, default: u16) -> u16 {
    GLOBAL
        .get()
        .map(|c| c.get_u16(key, default))
        .unwrap_or(default)
}

/// 读取布尔配置（回退默认值）
pub fn get_bool(key: &str, default: bool) -> bool {
    GLOBAL
        .get()
        .map(|c| c.get_bool(key, default))
        .unwrap_or(default)
}

/// 解析 .env 风格配置文本（支持 # 注释、空行、可选引号、trim）
fn parse_env(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim();
        if key.is_empty() {
            continue;
        }
        let mut value = line[eq + 1..].trim().to_string();
        // 去除可选引号
        if value.len() >= 2 {
            let first = value.chars().next().unwrap();
            let last = value.chars().last().unwrap();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                value = value[1..value.len() - 1].to_string();
            }
        }
        map.insert(key.to_string(), value);
    }
    map
}

/// 以模板为骨架渲染配置（保留注释/顺序，替换已有键的值，追加新键）
fn render_from_template(template: &str, map: &HashMap<String, String>) -> String {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = String::new();
    for line in template.lines() {
        let trimmed = line.trim();
        let mut replaced = false;
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim().to_string();
                if !key.is_empty() {
                    seen.insert(key.clone());
                    if let Some(v) = map.get(&key) {
                        out.push_str(&format!("{}={}", key, v));
                        replaced = true;
                    }
                }
            }
        }
        if !replaced {
            out.push_str(line);
        }
        out.push('\n');
    }
    for (k, v) in map {
        if !seen.contains(k) {
            out.push_str(&format!("{}={}\n", k, v));
        }
    }
    out
}

/// 内置默认模板（模板文件缺失时回退使用）
fn build_template(map: &HashMap<String, String>) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("# homeTier 应用配置文件".to_string());
    lines.push("# 修改保存后热更新生效（自动检测，无需重启）。".to_string());
    lines.push("# 优先级：配置文件 > 数据库设置 > 内置默认值。".to_string());
    lines.push("".to_string());
    lines.push("# ===== 端口类配置 =====（修改后下次 daemon 启动生效）".to_string());
    push_key(&mut lines, map, KEY_DAEMON_IPC_PORT, "homeTier daemon IPC 端口", "15889", "整数，1-65535");
    push_key(&mut lines, map, KEY_EASYTIER_RPC_PORT, "easytier-core RPC 端口", "15888", "整数，1-65535");
    lines.push("".to_string());
    lines.push("# ===== 网络配置 =====（修改后下次使用生效）".to_string());
    push_key(&mut lines, map, KEY_FILE_SERVER_PORT_BASE, "文件服务器端口基数（实际端口 = 基数 + space_id % 1000）", "19000", "整数，建议 1024-65535");
    push_key(&mut lines, map, KEY_DEFAULT_SPACE_IP, "新建空间的默认虚拟 IPv4 地址", "10.144.144.10", "IPv4 地址");
    lines.push("".to_string());
    lines.push("# ===== 更新与下载配置 =====（修改后下次使用生效）".to_string());
    push_key(&mut lines, map, KEY_GITHUB_API, "EasyTier GitHub Releases API 地址", "https://api.github.com/repos/EasyTier/EasyTier/releases", "URL");
    push_key(&mut lines, map, KEY_GITHUB_MIRROR, "下载镜像前缀（留空则直连 GitHub）", "https://ghproxy.top", "URL，可为空");
    lines.push("".to_string());
    lines.push("# ===== 业务配置 =====（立即生效）".to_string());
    push_key(&mut lines, map, "RELAY_NETWORK_PREFIX", "中继网络前缀（配合 EasyTier 转发白名单）", "homeTier_", "字符串");
    push_key(&mut lines, map, KEY_LOG_ENABLED, "日志开关", "1", "枚举：1=开启，0=关闭");
    lines.push("".to_string());
    lines.join("\n")
}

fn push_key(lines: &mut Vec<String>, map: &HashMap<String, String>, key: &str, desc: &str, default: &str, enum_note: &str) {
    lines.push(format!("# {}（默认值: {}；{}）", desc, default, enum_note));
    let value = map.get(key).cloned().unwrap_or_else(|| default.to_string());
    lines.push(format!("{}={}", key, value));
    lines.push("".to_string());
}
