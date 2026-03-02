use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

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
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from).map(|home| home.join(".cache")))
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
