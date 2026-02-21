use crate::platform::non_activating::windows::show_window_non_activating;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::app::config::AppConfig;
use crate::platform::system_info;
use tauri::{App, LogicalSize, Manager, Position, WindowEvent};
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
                        if let Some((left, top, right, bottom)) = data {
                            let (screen_left, screen_top, screen_right, screen_bottom) =
                                system_info::caret::get_monitor_bounds_by_point(app, left, bottom);

                            let (win_x, win_y) = compute_best_window_position(
                                left, top, right, bottom,
                                window_width, window_height,
                                spacing,
                                screen_left, screen_top, screen_right, screen_bottom,
                            );

                            let _ = window.set_position(Position::Logical((win_x, win_y).into()));
                        }

                        show_window_non_activating(&window); // 不激活显示
                    }

                    #[cfg(not(target_os = "windows"))]
                    let _ = window.show(); // 其他平台保持原有逻辑
                }
            }
        });
}

// 在 init_register_shortcut 函数内部或作为辅助函数
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


    // 定义候选位置生成器：每个候选返回 (x, y) 和是否优先考虑
    let candidates: [fn(&LayoutContext) -> (i32, i32); 4] = [
        position_bottom,
        position_top,
        position_left,
        position_right,
    ];

    // 尝试每个候选位置，检查是否遮挡光标
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

    // 默认返回下方位置（理论上不会执行到这里）
    let mut win_x = cursor_left;
    let mut win_y = cursor_bottom + spacing;
    if win_x + window_width > screen_right { win_x = screen_right - window_width; }
    if win_x < screen_left { win_x = screen_left; }
    if win_y + window_height > screen_bottom { win_y = screen_bottom - window_height; }
    if win_y < screen_top { win_y = screen_top; }
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

/// 下方
fn position_bottom(ctx: &LayoutContext) -> (i32, i32) {
    let mut win_x = ctx.cursor_left;
    let mut win_y = ctx.cursor_bottom + ctx.spacing;

    // 水平修正
    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    // 垂直修正
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }
    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }

    (win_x, win_y)
}

/// 上方
fn position_top(ctx: &LayoutContext) -> (i32, i32) {
    let mut win_x = ctx.cursor_left;
    let mut win_y = ctx.cursor_top - ctx.window_height - ctx.spacing;

    // 水平修正
    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    // 垂直修正
    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    (win_x, win_y)
}

/// 左侧
fn position_left(ctx: &LayoutContext) -> (i32, i32) {
    let cursor_center_y = (ctx.cursor_top + ctx.cursor_bottom) / 2;

    let mut win_y = cursor_center_y - ctx.window_height / 2;
    let mut win_x = ctx.cursor_left - ctx.window_width - ctx.spacing;

    // 垂直修正
    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    // 水平修正
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }
    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }

    (win_x, win_y)
}

/// 右侧
fn position_right(ctx: &LayoutContext) -> (i32, i32) {
    let cursor_center_y = (ctx.cursor_top + ctx.cursor_bottom) / 2;

    let mut win_y = cursor_center_y - ctx.window_height / 2;
    let mut win_x = ctx.cursor_right + ctx.spacing;

    // 垂直修正
    if win_y < ctx.screen_top {
        win_y = ctx.screen_top;
    }
    if win_y + ctx.window_height > ctx.screen_bottom {
        win_y = ctx.screen_bottom - ctx.window_height;
    }

    // 水平修正
    if win_x + ctx.window_width > ctx.screen_right {
        win_x = ctx.screen_right - ctx.window_width;
    }
    if win_x < ctx.screen_left {
        win_x = ctx.screen_left;
    }

    (win_x, win_y)
}


// /// 监听窗口失去焦点事件
// pub fn init_hide_register_shortcut_event(app: &App) {
//     if let Some(window) = app.get_window("index") {
//         let window_clone = window.clone();
//
//         window.on_window_event(move |event| {
//             if let WindowEvent::Focused(false) = event {
//                 println!("触发失去焦点事件");
//                 let _ = window_clone.hide();
//             }
//         })
//     }
// }

// 需要在应用启动时托管的状态
// pub struct ShortcutState {
//     pub(crate) auto_hide: AtomicBool,       // 当前是否允许自动隐藏（窗口显示时设为 true）
//     pub(crate) listener_added: AtomicBool,  // 是否已添加失去焦点监听
// }
//
// pub fn init_register_shortcut(app: &App) {
//     // 从 app 中读取配置，只复制基本类型或 owned 数据
//     let config = app.state::<AppConfig>();
//     let shortcut = config.hotkey.trim().to_string(); // 复制为 owned String
//     let window_width = config.clipboard_window_width;
//     let window_height = config.clipboard_window_height;
//     let spacing = config.clipboard_window_spacing;
//
//     // 注册全局快捷键 —— 闭包是 'static 的，不能捕获 &app
//     let _ = app.global_shortcut().on_shortcut(shortcut.as_str(), move |app_handle, _, _| {
//         // ✅ 正确：通过回调参数 app_handle 获取窗口和状态，不再依赖外部的 &app
//         if let Some(window) = app_handle.get_window("index") {
//             // 每次回调都重新获取状态（State 是线程安全的智能指针）
//             let state: tauri::State<ShortcutState> = app_handle.state();
//
//             if let Ok(false) = window.is_visible() {
//                 // --- 窗口不可见：显示并启用自动隐藏 ---
//                 let _ = window.set_size(LogicalSize::new(
//                     window_width.max(200) as f64,
//                     window_height.max(120) as f64,
//                 ));
//
//                 #[cfg(target_os = "windows")]
//                 {
//                     // 窗口定位逻辑（使用从外部复制来的 window_width, spacing 等）
//                     let data = system_info::caret::get_ui_automation_pos();
//                     if let Some((left, _top, _right, bottom)) = data {
//                         let monitor_bounds =
//                             system_info::caret::get_monitor_bounds_by_point(&app_handle, left, bottom);
//                         let (screen_left, screen_top, screen_right, screen_bottom) = monitor_bounds;
//
//                         let mut win_x = left;
//                         if win_x + window_width > screen_right { win_x = screen_right - window_width; }
//                         if win_x < screen_left { win_x = screen_left; }
//
//                         let mut win_y = bottom + spacing;
//                         if win_y + window_height > screen_bottom { win_y = screen_bottom - window_height; }
//                         if win_y < screen_top { win_y = screen_top; }
//
//                         let _ = window.set_position(Position::Logical((win_x, win_y).into()));
//                     }
//                 }
//
//                 // 启用自动隐藏标志
//                 state.auto_hide.store(true, Ordering::SeqCst);
//
//                 // 确保只添加一次失去焦点监听
//                 if !state.listener_added.load(Ordering::SeqCst) {
//                     let window_clone = window.clone(); // 为事件闭包克隆窗口
//                     window.on_window_event(move |event| {
//                         if let WindowEvent::Focused(false) = event {
//                             // ✅ 在事件闭包内通过 window_clone 重新获取状态
//                             let state: tauri::State<ShortcutState> = window_clone.state();
//                             if state.auto_hide.load(Ordering::SeqCst) {
//                                 let _ = window_clone.hide();
//                                 state.auto_hide.store(false, Ordering::SeqCst);
//                             }
//                         }
//                     });
//                     // 标记监听已添加（通过当前作用域的 state 修改全局状态）
//                     state.listener_added.store(true, Ordering::SeqCst);
//                 }
//
//                 let _ = window.show();
//             } else {
//                 // --- 窗口可见：主动隐藏，并禁用自动隐藏 ---
//                 state.auto_hide.store(false, Ordering::SeqCst);
//                 let _ = window.hide();
//             }
//         }
//     });
// }