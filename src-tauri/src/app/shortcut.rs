use crate::app::config::AppConfig;
use crate::plugins::system_info;
use tauri::{App, LogicalSize, Manager, Position};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub fn init_register_shortcut(app: &App) {
    let config = app.state::<AppConfig>();
    // let shortcut = config.hotkey.trim().to_lowercase();
    //
    // if !app.global_shortcut().is_registered(shortcut.as_str()) {
    //     let _ = app.global_shortcut().register(shortcut.as_str());
    // }
    let shortcut = config.hotkey.trim().to_string();
    let _ = app.global_shortcut().unregister(shortcut.as_str());

    let window_width = config.clipboard_window_width;
    let window_height = config.clipboard_window_height;
    let spacing = config.clipboard_window_spacing;

    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut.as_str(), move |app, _, _| {
            if let Some(window) = app.get_window("index") {
                if let Ok(false) = window.is_visible() {
                    let _ = window.set_size(LogicalSize::new(
                        window_width.max(200) as f64,
                        window_height.max(120) as f64,
                    ));

                    #[cfg(target_os = "windows")]
                    {
                        let data = system_info::caret::get_ui_automation_pos();
                        if let Some((left, _top, _right, bottom)) = data {
                            let monitor_bounds =
                                system_info::caret::get_monitor_bounds_by_point(app, left, bottom);

                            let (screen_left, screen_top, screen_right, screen_bottom) =
                                monitor_bounds;

                            let mut win_x = left;
                            if win_x + window_width > screen_right {
                                win_x = screen_right - window_width;
                            }
                            if win_x < screen_left {
                                win_x = screen_left;
                            }

                            let mut win_y = bottom + spacing;
                            if win_y + window_height > screen_bottom {
                                win_y = screen_bottom - window_height;
                            }
                            if win_y < screen_top {
                                win_y = screen_top;
                            }

                            let _ = window.set_position(Position::Logical((win_x, win_y).into()));
                        }
                    }

                    let _ = window.show();
                } else {
                    let _ = window.hide();
                }
            }
        });
}
