use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub hotkey: String,
    pub clipboard_window_width: i32,
    pub clipboard_window_height: i32,
    pub clipboard_window_spacing: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "f4".to_string(),
            clipboard_window_width: 320,
            clipboard_window_height: 400,
            clipboard_window_spacing: 10,
        }
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

    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                return config;
            }
        }
    }

    let default_config = AppConfig::default();
    if let Ok(content) = toml::to_string_pretty(&default_config) {
        let _ = fs::write(&path, content);
    }
    default_config
}
