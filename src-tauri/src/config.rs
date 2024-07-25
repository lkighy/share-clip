use tauri::{App, AppHandle, Manager};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use crate::utils::toggle_window_visibility;

/// 初始化快捷键注册
pub fn init_register_shortcut(app: &App) {
    let shortcut = "ctrl+alt+v";
    if app.global_shortcut().is_registered(shortcut) {
        let _ = app.global_shortcut().register(shortcut);
    }
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, |app, shortcut, _| {
            if let Some(window) = app.get_webview_window("index") {
                toggle_window_visibility(&window);
            }
        });
    let shortcut = "f12";
    if app.global_shortcut().is_registered(shortcut) {
        let _ = app.global_shortcut().register(shortcut);
    }
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, |app, shortcut, _| {
            if let Some(window) = app.get_webview_window("index") {
                if let Ok(true) = window.is_visible() {
                    window.open_devtools();
                }
            }
        });
}

/// 初始化菜单栏
pub fn init_menu(app: &App) {
    // 设置剪切板
    let clipboard_item = MenuItemBuilder::with_id("clipboard", "剪切板").build(app).expect("创建接切板列表");
    let setting_item = MenuItemBuilder::with_id("setting", "设置").build(app).expect("创建设置列表");
    let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app).expect("创建退出列表");
    let menu = MenuBuilder::new(app)
        .items(&[&clipboard_item, &setting_item, &quit_item])
        .build().expect("创建菜单列表失败");
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "clipboard" => {
                if let Some(window) = app.get_webview_window("index") {
                    toggle_window_visibility(&window);
                }
            }
            "quit" => app.exit(0),
            "setting" => {
                if let Some(window) = app.get_webview_window("setting") {
                    toggle_window_visibility(&window);
                }
            }
            _ => (),
        })
        .build(app).expect("初始化菜单列表失败！");
}

// 绑定的数据有哪些呢
// 1. 注册快捷键
/// 关键字
pub fn register_clip_shortcut_key(app: &AppHandle, shortcut: &str) -> bool {
    if !app.global_shortcut().is_registered(shortcut) {
        return false;
    }

    //
    // 绑定按钮功能，该功能应该是用于监听复制的快捷键
    // TODO: 重置监听快捷键

    true
}
