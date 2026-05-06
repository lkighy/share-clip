use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

use parking_lot::RwLock;

const APP_IDENTIFIER: &str = "com.lkighy.share-clip";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(alias = "hotkey")]
    pub shortcut: String,
    pub clipboard_window_width: i32,
    pub clipboard_window_height: i32,
    pub clipboard_window_spacing: i32,
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
    // 是否启用共享服务器
    pub enable_share_server: bool,
    // 共享服务器绑定IP
    pub share_server_bind_ip: String,
    // 共享服务端口（文件列表/下载/diff）
    pub share_server_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct AppConfigUpdate {
    pub shortcut: Option<String>,
    pub clipboard_window_width: Option<i32>,
    pub clipboard_window_height: Option<i32>,
    pub clipboard_window_spacing: Option<i32>,
    pub auto_cleanup_invalid_clipboard_data: Option<bool>,
    pub cache_dir: Option<String>,
    pub remote_cache_dir: Option<String>,
    pub cleanup_after_days: Option<Option<u64>>,
    pub max_items: Option<Option<usize>>,
    pub default_share_text: Option<bool>,
    pub default_share_image: Option<bool>,
    pub default_share_file: Option<bool>,
    pub default_share_folder: Option<bool>,
    pub enable_share_server: Option<bool>,
    pub share_server_bind_ip: Option<String>,
    pub share_server_port: Option<u16>,
}

impl AppConfigUpdate {
    pub fn apply(self, config: &mut AppConfig) {
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
        if let Some(value) = self.enable_share_server {
            config.enable_share_server = value;
        }
        if let Some(value) = self.share_server_bind_ip {
            config.share_server_bind_ip = value;
        }
        if let Some(value) = self.share_server_port {
            config.share_server_port = value;
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            shortcut: "f4".to_string(),
            clipboard_window_width: 420,
            clipboard_window_height: 640,
            clipboard_window_spacing: 10,
            auto_cleanup_invalid_clipboard_data: true,
            cache_dir: default_cache_dir(),
            remote_cache_dir: "remote".to_string(),
            cleanup_after_days: None,
            max_items: None,
            default_share_text: true,
            default_share_image: false,
            default_share_file: false,
            default_share_folder: false,
            enable_share_server: false,
            share_server_bind_ip: "0.0.0.0".to_string(),
            share_server_port: 24800,
        }
    }
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
