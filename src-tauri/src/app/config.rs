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
    pub cache_dir: String,
    pub image_cache_threshold_bytes: u64,
    pub remote_cache_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            shortcut: "f4".to_string(),
            clipboard_window_width: 420,
            clipboard_window_height: 640,
            clipboard_window_spacing: 10,
            cache_dir: default_cache_dir(),
            image_cache_threshold_bytes: 512 * 1024,
            remote_cache_dir: "remote".to_string(),
        }
    }
}

fn default_cache_dir() -> String {
    if cfg!(target_os = "windows") {
        // Windows 使用系统临时目录，例如 C:\Users\<用户名>\AppData\Local\Temp\<应用标识>
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
        // Linux 遵循 XDG 规范
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
