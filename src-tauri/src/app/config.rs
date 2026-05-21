use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;

const APP_IDENTIFIER: &str = "com.lkighy.share-clip";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    // 本机设备稳定 ID，用于远程设备识别
    pub local_device_id: String,
    // 本机设备显示名称，可由用户修改
    pub local_device_name: String,
    #[serde(alias = "hotkey")]
    pub shortcut: String,
    pub clipboard_window_width: i32,
    pub clipboard_window_height: i32,
    pub clipboard_window_spacing: i32,
    pub theme_mode: String,
    pub share_files_view_mode: String,
    pub share_files_item_zoom: i32,
    // 普通文本超过该字节数时不保存完整内容
    pub clipboard_text_max_bytes: u64,
    // 单个 HTML/RTF 格式超过该字节数时降级丢弃该格式
    pub clipboard_rich_format_max_bytes: u64,
    // 单条富文本剪切板记录可保存的总字节数上限
    pub clipboard_total_max_bytes: u64,
    // 是否自动清理失效数据
    pub auto_cleanup_invalid_clipboard_data: bool,
    // 缓存目录
    pub cache_dir: String,
    // 远程数据缓存目录
    pub remote_cache_dir: String,
    /// 如果为 Some(days)，则自动清理超过 days 天的条目
    pub cleanup_after_days: Option<u64>,
    /// 如果为 Some(n)，则最多保留 n 条记录（None 表示无限制）
    pub max_items: Option<usize>,
    // 是否默认分享文本，默认为 true
    pub default_share_text: bool,
    // 是否默认分享复制的图片，默认为 false
    pub default_share_image: bool,
    // 是否默认分享文件，默认为 false
    pub default_share_file: bool,
    // 是否默认分享文件夹，默认为 false
    pub default_share_folder: bool,
    // 剪贴板来源文件变化时标记对应临时分享失效
    pub unshare_on_clipboard_change: bool,
    // 是否在应用启动时自动启动共享服务器
    #[serde(alias = "enable_share_server")]
    pub auto_start_share_server: bool,
    // 共享服务器绑定IP
    pub share_server_bind_ip: String,
    // 共享服务端口（文件列表/下载/diff）
    pub share_server_port: u16,
    // 是否启用共享服务器密码
    pub share_server_password_enabled: bool,
    // 共享服务器密码哈希（后续授权链路使用）
    pub share_server_password_hash: Option<String>,
    // 授权模式：0=自动授权，1=需要确认
    pub share_server_auth_mode: i32,
    // 是否允许其他设备通过局域网扫描发现本机
    pub lan_discovery_enabled: bool,
    // 是否允许浏览器访问
    pub browser_access_enabled: bool,
    // Web 访问是否默认需要授权
    pub web_access_auth_required: bool,
    // 是否允许使用固定 Web 访问密码授权
    pub web_access_password_enabled: bool,
    // 固定 Web 访问密码
    pub web_access_password: Option<String>,
    // 是否允许通过桌面端临时确认授权
    pub web_access_temp_approval_enabled: bool,
    // Web 授权 Cookie 有效期，默认 1 小时
    pub web_access_cookie_ttl_seconds: u64,
    // Web 权限粒度开关
    pub web_access_scope_files: bool,
    pub web_access_scope_clipboard_list: bool,
    pub web_access_scope_clipboard_content: bool,
    pub web_access_scope_download: bool,
    // 是否允许客户端同步访问
    pub sync_access_enabled: bool,
    // 收到连接申请时是否主动弹出确认浮窗
    pub popup_on_inbound_request: bool,
}

#[derive(Debug, Deserialize)]
pub struct AppConfigUpdate {
    pub local_device_id: Option<String>,
    pub local_device_name: Option<String>,
    pub shortcut: Option<String>,
    pub clipboard_window_width: Option<i32>,
    pub clipboard_window_height: Option<i32>,
    pub clipboard_window_spacing: Option<i32>,
    pub theme_mode: Option<String>,
    pub share_files_view_mode: Option<String>,
    pub share_files_item_zoom: Option<i32>,
    pub clipboard_text_max_bytes: Option<u64>,
    pub clipboard_rich_format_max_bytes: Option<u64>,
    pub clipboard_total_max_bytes: Option<u64>,
    pub auto_cleanup_invalid_clipboard_data: Option<bool>,
    pub cache_dir: Option<String>,
    pub remote_cache_dir: Option<String>,
    pub cleanup_after_days: Option<Option<u64>>,
    pub max_items: Option<Option<usize>>,
    pub default_share_text: Option<bool>,
    pub default_share_image: Option<bool>,
    pub default_share_file: Option<bool>,
    pub default_share_folder: Option<bool>,
    pub unshare_on_clipboard_change: Option<bool>,
    pub auto_start_share_server: Option<bool>,
    #[serde(alias = "enable_share_server")]
    pub enable_share_server: Option<bool>,
    pub share_server_bind_ip: Option<String>,
    pub share_server_port: Option<u16>,
    pub share_server_password_enabled: Option<bool>,
    pub share_server_password_hash: Option<Option<String>>,
    pub share_server_auth_mode: Option<i32>,
    pub lan_discovery_enabled: Option<bool>,
    pub browser_access_enabled: Option<bool>,
    pub web_access_auth_required: Option<bool>,
    pub web_access_password_enabled: Option<bool>,
    pub web_access_password: Option<Option<String>>,
    pub web_access_temp_approval_enabled: Option<bool>,
    pub web_access_cookie_ttl_seconds: Option<u64>,
    pub web_access_scope_files: Option<bool>,
    pub web_access_scope_clipboard_list: Option<bool>,
    pub web_access_scope_clipboard_content: Option<bool>,
    pub web_access_scope_download: Option<bool>,
    pub sync_access_enabled: Option<bool>,
    pub popup_on_inbound_request: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LanDiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub base_url: String,
    pub sync_access_enabled: bool,
    pub password_required: bool,
    pub auth_mode: i32,
}

#[derive(Debug, Deserialize)]
struct LanDiscoveryHttpResponse {
    device_id: String,
    device_name: String,
    port: u16,
    sync_access_enabled: bool,
    password_required: bool,
    auth_mode: i32,
}

impl AppConfigUpdate {
    pub fn apply(self, config: &mut AppConfig) {
        if let Some(value) = self.local_device_id {
            let value = value.trim();
            if !value.is_empty() {
                config.local_device_id = value.to_string();
            }
        }
        if let Some(value) = self.local_device_name {
            let value = value.trim();
            if !value.is_empty() {
                config.local_device_name = value.to_string();
            }
        }
        if let Some(value) = self.shortcut {
            config.shortcut = value;
        }
        if let Some(value) = self.clipboard_window_width {
            config.clipboard_window_width = value;
        }
        if let Some(value) = self.clipboard_window_height {
            config.clipboard_window_height = value;
        }
        if let Some(value) = self.clipboard_window_spacing {
            config.clipboard_window_spacing = value;
        }
        if let Some(value) = self.theme_mode {
            let value = value.trim();
            if matches!(value, "system" | "light" | "dark") {
                config.theme_mode = value.to_string();
            }
        }
        if let Some(value) = self.share_files_view_mode {
            let value = value.trim();
            if !value.is_empty() {
                config.share_files_view_mode = value.to_string();
            }
        }
        if let Some(value) = self.share_files_item_zoom {
            config.share_files_item_zoom = value;
        }
        if let Some(value) = self.clipboard_text_max_bytes {
            config.clipboard_text_max_bytes = value;
        }
        if let Some(value) = self.clipboard_rich_format_max_bytes {
            config.clipboard_rich_format_max_bytes = value;
        }
        if let Some(value) = self.clipboard_total_max_bytes {
            config.clipboard_total_max_bytes = value;
        }
        if let Some(value) = self.auto_cleanup_invalid_clipboard_data {
            config.auto_cleanup_invalid_clipboard_data = value;
        }
        if let Some(value) = self.cache_dir {
            config.cache_dir = value;
        }
        if let Some(value) = self.remote_cache_dir {
            config.remote_cache_dir = value;
        }
        if let Some(value) = self.cleanup_after_days {
            config.cleanup_after_days = value;
        }
        if let Some(value) = self.max_items {
            config.max_items = value;
        }
        if let Some(value) = self.default_share_text {
            config.default_share_text = value;
        }
        if let Some(value) = self.default_share_image {
            config.default_share_image = value;
        }
        if let Some(value) = self.default_share_file {
            config.default_share_file = value;
        }
        if let Some(value) = self.default_share_folder {
            config.default_share_folder = value;
        }
        if let Some(value) = self.unshare_on_clipboard_change {
            config.unshare_on_clipboard_change = value;
        }
        if let Some(value) = self.auto_start_share_server.or(self.enable_share_server) {
            config.auto_start_share_server = value;
        }
        if let Some(value) = self.share_server_bind_ip {
            config.share_server_bind_ip = value;
        }
        if let Some(value) = self.share_server_port {
            config.share_server_port = value;
        }
        if let Some(value) = self.share_server_password_enabled {
            config.share_server_password_enabled = value;
        }
        if let Some(value) = self.share_server_password_hash {
            config.share_server_password_hash = value;
        }
        if let Some(value) = self.share_server_auth_mode {
            config.share_server_auth_mode = value;
        }
        if let Some(value) = self.lan_discovery_enabled {
            config.lan_discovery_enabled = value;
        }
        if let Some(value) = self.browser_access_enabled {
            config.browser_access_enabled = value;
        }
        if let Some(value) = self.web_access_auth_required {
            config.web_access_auth_required = value;
        }
        if let Some(value) = self.web_access_password_enabled {
            config.web_access_password_enabled = value;
        }
        if let Some(value) = self.web_access_password {
            config.web_access_password = value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
        if let Some(value) = self.web_access_temp_approval_enabled {
            config.web_access_temp_approval_enabled = value;
        }
        if let Some(value) = self.web_access_cookie_ttl_seconds {
            config.web_access_cookie_ttl_seconds = value.max(60);
        }
        if let Some(value) = self.web_access_scope_files {
            config.web_access_scope_files = value;
        }
        if let Some(value) = self.web_access_scope_clipboard_list {
            config.web_access_scope_clipboard_list = value;
        }
        if let Some(value) = self.web_access_scope_clipboard_content {
            config.web_access_scope_clipboard_content = value;
        }
        if let Some(value) = self.web_access_scope_download {
            config.web_access_scope_download = value;
        }
        if let Some(value) = self.sync_access_enabled {
            config.sync_access_enabled = value;
        }
        if let Some(value) = self.popup_on_inbound_request {
            config.popup_on_inbound_request = value;
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            local_device_id: default_device_id(),
            local_device_name: default_device_name(),
            shortcut: "f4".to_string(),
            clipboard_window_width: 420,
            clipboard_window_height: 640,
            clipboard_window_spacing: 10,
            theme_mode: "system".to_string(),
            share_files_view_mode: "icons".to_string(),
            share_files_item_zoom: 100,
            clipboard_text_max_bytes: 4 * 1024 * 1024,
            clipboard_rich_format_max_bytes: 8 * 1024 * 1024,
            clipboard_total_max_bytes: 16 * 1024 * 1024,
            auto_cleanup_invalid_clipboard_data: true,
            cache_dir: default_cache_dir(),
            remote_cache_dir: "remote".to_string(),
            cleanup_after_days: None,
            max_items: None,
            default_share_text: true,
            default_share_image: false,
            default_share_file: false,
            default_share_folder: false,
            unshare_on_clipboard_change: true,
            auto_start_share_server: false,
            share_server_bind_ip: "0.0.0.0".to_string(),
            share_server_port: 24800,
            share_server_password_enabled: false,
            share_server_password_hash: None,
            share_server_auth_mode: 1,
            lan_discovery_enabled: true,
            browser_access_enabled: true,
            web_access_auth_required: true,
            web_access_password_enabled: false,
            web_access_password: None,
            web_access_temp_approval_enabled: true,
            web_access_cookie_ttl_seconds: 3600,
            web_access_scope_files: true,
            web_access_scope_clipboard_list: true,
            web_access_scope_clipboard_content: true,
            web_access_scope_download: true,
            sync_access_enabled: true,
            popup_on_inbound_request: false,
        }
    }
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| std::env::var("USER"))
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "本机设备".to_string())
}

fn default_device_id() -> String {
    let parts = [
        APP_IDENTIFIER.to_string(),
        machine_fingerprint().unwrap_or_default(),
        std::env::consts::OS.to_string(),
        std::env::consts::ARCH.to_string(),
        default_device_name(),
        std::env::var("USERNAME").unwrap_or_default(),
        std::env::var("USER").unwrap_or_default(),
        std::env::var("USERPROFILE").unwrap_or_default(),
        std::env::var("HOME").unwrap_or_default(),
    ];
    let input = parts.join("|");
    let hash = blake3::hash(input.as_bytes()).to_hex().to_string();
    format!("device-{}", &hash[..20])
}

fn machine_fingerprint() -> Option<String> {
    if cfg!(target_os = "windows") {
        return Command::new("reg")
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| {
                text.lines()
                    .find(|line| line.contains("MachineGuid"))
                    .and_then(|line| line.split_whitespace().last())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });
    }

    if cfg!(target_os = "macos") {
        return Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| {
                text.lines()
                    .find(|line| line.contains("IOPlatformUUID"))
                    .and_then(|line| line.split('"').nth(3))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });
    }

    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .into_iter()
        .find_map(|path| {
            fs::read_to_string(path)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn default_cache_dir() -> String {
    if cfg!(target_os = "windows") {
        // Windows uses system temp path, e.g. C:\Users\<user>\AppData\Local\Temp\<app-id>
        std::env::temp_dir()
            .join(APP_IDENTIFIER)
            .to_string_lossy()
            .into_owned()
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Caches").join(APP_IDENTIFIER))
            .unwrap_or_else(|| PathBuf::from("cache"))
            .to_string_lossy()
            .into_owned()
    } else {
        // Linux follows XDG spec
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".cache"))
            })
            .map(|dir| dir.join(APP_IDENTIFIER))
            .unwrap_or_else(|| PathBuf::from("cache"))
            .to_string_lossy()
            .into_owned()
    }
}

pub fn private_ipv4_addresses() -> io::Result<Vec<Ipv4Addr>> {
    let mut ips = BTreeSet::new();
    let addrs = if_addrs::get_if_addrs().map_err(io::Error::other)?;
    for iface in addrs {
        if let std::net::IpAddr::V4(ipv4) = iface.ip() {
            if is_private_ipv4(ipv4) {
                ips.insert(ipv4);
            }
        }
    }
    Ok(ips.into_iter().collect())
}

pub async fn scan_lan_devices(
    port: u16,
    local_device_id: &str,
) -> io::Result<Vec<LanDiscoveredDevice>> {
    let local_ips = private_ipv4_addresses()?;
    let semaphore = Arc::new(Semaphore::new(64));
    let mut handles = Vec::new();

    for local_ip in &local_ips {
        let [a, b, c, local_host] = local_ip.octets();
        for host in 1..=254u8 {
            if host == local_host {
                continue;
            }
            let ip = Ipv4Addr::new(a, b, c, host);
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(io::Error::other)?;
            handles.push(tokio::spawn(async move {
                let result = probe_lan_device(ip, port).await;
                drop(permit);
                result
            }));
        }
    }

    let mut by_device_id = BTreeMap::new();
    for handle in handles {
        if let Ok(Ok(device)) = handle.await {
            if device.device_id != local_device_id {
                by_device_id.insert(device.device_id.clone(), device);
            }
        }
    }

    Ok(by_device_id.into_values().collect())
}

async fn probe_lan_device(ip: Ipv4Addr, port: u16) -> io::Result<LanDiscoveredDevice> {
    let mut stream = timeout(Duration::from_millis(350), TcpStream::connect((ip, port)))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timed out"))??;

    let request = "GET /api/client/discovery HTTP/1.1\r\nHost: share-clip.local\r\nConnection: close\r\nAccept: application/json\r\n\r\n";
    timeout(
        Duration::from_millis(350),
        stream.write_all(request.as_bytes()),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "write timed out"))??;

    let mut buf = Vec::new();
    timeout(Duration::from_millis(700), stream.read_to_end(&mut buf))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timed out"))??;

    parse_lan_discovery_response(ip, port, &buf)
}

fn parse_lan_discovery_response(
    ip: Ipv4Addr,
    fallback_port: u16,
    response: &[u8],
) -> io::Result<LanDiscoveredDevice> {
    let text = String::from_utf8_lossy(response);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid http response"))?;
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not discovered"));
    }
    let body = body.trim();
    let payload: LanDiscoveryHttpResponse = serde_json::from_str(body).map_err(io::Error::other)?;
    let port = if payload.port == 0 {
        fallback_port
    } else {
        payload.port
    };
    let ip_text = ip.to_string();
    Ok(LanDiscoveredDevice {
        device_id: payload.device_id,
        device_name: payload.device_name,
        ip: ip_text.clone(),
        port,
        base_url: format!("http://{ip_text}:{port}"),
        sync_access_enabled: payload.sync_access_enabled,
        password_required: payload.password_required,
        auth_mode: payload.auth_mode,
    })
}

pub fn is_private_ipv4(ipv4: Ipv4Addr) -> bool {
    let octets = ipv4.octets();
    !ipv4.is_loopback()
        && (octets[0] == 10
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            || (octets[0] == 192 && octets[1] == 168))
}

fn config_file_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            return dir.join("config.toml");
        }
    }
    PathBuf::from("config.toml")
}

pub fn load_or_create_config() -> AppConfig {
    let path = config_file_path();
    let default_config = AppConfig::default();

    if !path.exists() {
        let _ = update_config_file(&default_config);
        return default_config;
    }

    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(config) = toml::from_str::<AppConfig>(&content) {
            let _ = update_config_file(&config);
            return config;
        }
    }

    let _ = update_config_file(&default_config);
    default_config
}

pub fn update_config_file(config: &AppConfig) -> io::Result<()> {
    let content = toml::to_string_pretty(config)
        .map_err(|err| io::Error::other(format!("serialize config failed: {err}")))?;

    fs::write(config_file_path(), content)
}

#[derive(Debug)]
pub struct AppConfigStore {
    inner: RwLock<AppConfig>,
}

impl AppConfigStore {
    pub fn load() -> Self {
        Self {
            inner: RwLock::new(load_or_create_config()),
        }
    }

    pub fn get(&self) -> AppConfig {
        self.inner.read().clone()
    }

    pub fn update(&self, config: AppConfig) -> io::Result<AppConfig> {
        update_config_file(&config)?;
        *self.inner.write() = config.clone();
        Ok(config)
    }

    pub fn update_with(&self, update: AppConfigUpdate) -> io::Result<AppConfig> {
        let mut config = self.inner.read().clone();
        update.apply(&mut config);
        self.update(config)
    }
}
