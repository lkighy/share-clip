use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{App, Manager};

pub fn init_menu(app: &App) {
    let clipboard_item = MenuItemBuilder::with_id("index", "剪切板")
        .build(app)
        .expect("创建列表 - 剪切板失败");
    let quit_item = MenuItemBuilder::with_id("quit", "退出")
        .build(app)
        .expect("创建列表 - 退出失败");
    let menu = MenuBuilder::new(app)
        .items(&[&clipboard_item, &quit_item])
        .build()
        .expect("构建菜单列表失败");

    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "index" => {
                if let Some(window) = app.get_window("index") {
                    if let Ok(false) = window.is_visible() {
                        let _ = window.show();
                    } else {
                        let _ = window.hide();
                    }
                }
            }
            "quit" => app.exit(0),
            _ => (),
        })
        .build(app)
        .expect("初始化菜单列表失败");
}
