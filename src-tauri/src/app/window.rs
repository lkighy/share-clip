use crate::app::config::AppConfig;
#[cfg(target_os = "windows")]
use crate::plugins::non_activating::windows;
use tauri::{App, LogicalSize, Manager};

pub fn init_app(app: &mut App) {
    let config = app.state::<AppConfig>();

    if let Some(window) = app.get_window("index") {
        let _ = window.set_size(LogicalSize::new(
            config.clipboard_window_width.max(200) as f64,
            config.clipboard_window_height.max(120) as f64,
        ));

        #[cfg(target_os = "windows")]
        windows::init_non_activating_window(&window);
    };
}
