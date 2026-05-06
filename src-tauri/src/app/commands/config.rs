use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use std::collections::BTreeSet;

use crate::app::config::{AppConfig, AppConfigStore, AppConfigUpdate};
use crate::app::shortcuts::global::init_register_shortcut;
use crate::error::{ApiError, AppError};

#[tauri::command]
pub fn get_app_config(app: tauri::AppHandle) -> Result<AppConfig, ApiError> {
    let store = app.state::<AppConfigStore>();
    Ok(store.get())
}

#[tauri::command]
pub fn update_app_config(
    app: tauri::AppHandle,
    payload: AppConfigUpdate,
) -> Result<AppConfig, ApiError> {
    let store = app.state::<AppConfigStore>();
    let previous = store.get();
    let updated = store
        .update_with(payload)
        .map_err(|err| AppError::InvalidInput(err.to_string()))?;

    let _ = app.global_shortcut().unregister(previous.shortcut.trim());
    init_register_shortcut(&app);

    if let Some(window) = app.get_window("index") {
        let _ = window.set_size(tauri::LogicalSize::new(
            updated.clipboard_window_width.max(200) as f64,
            updated.clipboard_window_height.max(120) as f64,
        ));
    }

    let server_state = app.state::<crate::server::ServerState>();
    let was_running = server_state.is_running().unwrap_or(false);
    if updated.enable_share_server {
        if was_running {
            let _ = server_state.stop();
        }
        server_state
            .start(&updated.share_server_bind_ip, updated.share_server_port)
            .map_err(AppError::InvalidInput)?;
    } else if was_running {
        let _ = server_state.stop();
    }

    Ok(updated)
}

#[tauri::command]
pub fn get_share_server_ip_options() -> Result<Vec<String>, ApiError> {
    let mut ips = BTreeSet::new();
    ips.insert("127.0.0.1".to_string());
    ips.insert("0.0.0.0".to_string());

    let addrs = if_addrs::get_if_addrs().map_err(|e| AppError::InvalidInput(e.to_string()))?;
    for iface in addrs {
        if let std::net::IpAddr::V4(ipv4) = iface.ip() {
            if ipv4.is_loopback() {
                continue;
            }
            let octets = ipv4.octets();
            let is_private = octets[0] == 10
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168);
            if is_private {
                ips.insert(ipv4.to_string());
            }
        }
    }
    Ok(ips.into_iter().collect())
}
