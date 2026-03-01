use crate::app::config::AppConfig;
use crate::platform::system_info;
#[cfg(target_os = "windows")]
use crate::platform::non_activating::windows::show_window_non_activating;
use tauri::{App, LogicalSize, Manager, Position};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub fn init_register_shortcut(app: &App) {
    let config = app.state::<AppConfig>();
    let shortcut = config.shortcut.trim().to_string();
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
                        if let Some((left, top, right, bottom)) = data {
                            let (screen_left, screen_top, screen_right, screen_bottom) =
                                system_info::caret::get_monitor_bounds_by_point(app, left, bottom);

                            let (win_x, win_y) = compute_best_window_position(
                                left,
                                top,
                                right,
                                bottom,
                                window_width,
                                window_height,
                                spacing,
                                screen_left,
                                screen_top,
                                screen_right,
                                screen_bottom,
                            );

                            let _ = window.set_position(Position::Logical((win_x, win_y).into()));
                        }

                        show_window_non_activating(&window);
                    }

                    #[cfg(not(target_os = "windows"))]
                    let _ = window.show();
                }
            }
        });
}

fn compute_best_window_position(
    cursor_left: i32,
    cursor_top: i32,
    cursor_right: i32,
    cursor_bottom: i32,
    window_width: i32,
    window_height: i32,
    spacing: i32,
    screen_left: i32,
    screen_top: i32,
    screen_right: i32,
    screen_bottom: i32,
) -> (i32, i32) {
    let ctx = LayoutContext {
        window_width,
        window_height,
        spacing,
        screen_left,
        screen_top,
        screen_right,
        screen_bottom,
        cursor_left,
        cursor_top,
        cursor_right,
        cursor_bottom,
    };

    let candidates: [fn(&LayoutContext) -> (i32, i32); 4] = [
        position_bottom,
        position_top,
        position_left,
        position_right,
    ];

    for (i, candidate) in candidates.iter().enumerate() {
        let (win_x, win_y) = candidate(&ctx);

        let overlap = !(win_x + ctx.window_width <= ctx.cursor_left
            || win_x >= ctx.cursor_right
            || win_y + ctx.window_height <= ctx.cursor_top
            || win_y >= ctx.cursor_bottom);

        if !overlap || i == candidates.len() - 1 {
            return (win_x, win_y);
        }
    }

    let mut win_x = cursor_left;
    let mut win_y = cursor_bottom + spacing;
    if win_x + window_width > screen_right {
        win_x = screen_right - window_width;
    }
    if win_x < screen_left {
        win_x = screen_left;
    }
    if win_y + window_height > screen_bottom {
        win_y = screen_bottom - window_height;
    }
    if win_y < screen_top {
        win_y = screen_top;
    }
    (win_x, win_y)
}

struct LayoutContext {
    window_width: i32,
    window_height: i32,
    spacing: i32,
    screen_left: i32,
    screen_top: i32,
    screen_right: i32,
    screen_bottom: i32,
    cursor_left: i32,
    cursor_top: i32,
    cursor_right: i32,
    cursor_bottom: i32,
}

fn position_bottom(ctx: &LayoutContext) -> (i32, i32) {
    let mut win_x = ctx.cursor_left;
    let mut win_y = ctx.cursor_bottom + ctx.spacing;

    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }
    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }

    (win_x, win_y)
}

fn position_top(ctx: &LayoutContext) -> (i32, i32) {
    let mut win_x = ctx.cursor_left;
    let mut win_y = ctx.cursor_top - ctx.window_height - ctx.spacing;

    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    (win_x, win_y)
}

fn position_left(ctx: &LayoutContext) -> (i32, i32) {
    let cursor_center_y = (ctx.cursor_top + ctx.cursor_bottom) / 2;

    let mut win_y = cursor_center_y - ctx.window_height / 2;
    let mut win_x = ctx.cursor_left - ctx.window_width - ctx.spacing;

    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }
    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }

    (win_x, win_y)
}

fn position_right(ctx: &LayoutContext) -> (i32, i32) {
    let cursor_center_y = (ctx.cursor_top + ctx.cursor_bottom) / 2;

    let mut win_y = cursor_center_y - ctx.window_height / 2;
    let mut win_x = ctx.cursor_right + ctx.spacing;

    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    (win_x, win_y)
}
