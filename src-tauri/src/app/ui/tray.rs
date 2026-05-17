use crate::app::config::AppConfigStore;
use crate::app::ui::window::open_or_create_window;
use crate::models::window::WindowLabel;
use log::error;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager, Wry};

pub struct TrayMenuState {
    share_server_item: MenuItem<Wry>,
    connect_device_item: MenuItem<Wry>,
}

pub fn update_share_server_menu_label(app: &AppHandle) {
    let Ok(state) = app.state::<crate::server::ServerState>().is_running() else {
        return;
    };
    if let Some(menu_state) = app.try_state::<TrayMenuState>() {
        let _ = menu_state.share_server_item.set_text(if state {
            "关闭共享服务器"
        } else {
            "启动共享服务器"
        });
    }
}

pub fn update_connect_device_menu_pending_count(app: &AppHandle, count: u64) {
    if let Some(menu_state) = app.try_state::<TrayMenuState>() {
        let text = if count > 0 {
            format!("连接设备... ({count})")
        } else {
            "连接设备...".to_string()
        };
        let _ = menu_state.connect_device_item.set_text(text);
    }
}

pub fn init_menu(app: &App) {
    let clipboard_item = MenuItemBuilder::with_id("index", "剪贴板")
        .build(app)
        .expect("创建菜单项 - 剪贴板失败");
    let shared_files_item = MenuItemBuilder::with_id("shared-files", "分享文件")
        .build(app)
        .expect("创建菜单项 - 分享文件失败");
    let connect_device_item = MenuItemBuilder::with_id("connect-device", "连接设备...")
        .build(app)
        .expect("创建菜单项 - 连接设备失败");
    let app_config_item = MenuItemBuilder::with_id("app-config", "设置")
        .build(app)
        .expect("创建菜单项 - 设置失败");
    let share_server_running = app
        .state::<crate::server::ServerState>()
        .is_running()
        .unwrap_or(false);
    let share_server_item = MenuItemBuilder::with_id(
        "toggle-share-server",
        if share_server_running {
            "关闭共享服务器"
        } else {
            "启动共享服务器"
        },
    )
    .build(app)
    .expect("创建菜单项 - 共享服务器失败");
    let quit_item = MenuItemBuilder::with_id("quit", "退出")
        .build(app)
        .expect("创建菜单项 - 退出失败");

    app.manage(TrayMenuState {
        share_server_item: share_server_item.clone(),
        connect_device_item: connect_device_item.clone(),
    });

    let menu = MenuBuilder::new(app)
        .items(&[
            &clipboard_item,
            &shared_files_item,
            &connect_device_item,
            &app_config_item,
            &share_server_item,
            &quit_item,
        ])
        .build()
        .expect("构建托盘菜单失败");

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
            // "shared-files" => {
            //     if let Some(window) = app.get_window("shared-files") {
            //         if let Ok(false) = window.is_visible() {
            //             let _ = window.show();
            //         }
            //         let _ = window.set_focus();
            //     }
            // }
            // "app-config" => {
            //     if let Some(window) = app.get_window("app-config") {
            //         if let Ok(false) = window.is_visible() {
            //             let _ = window.show();
            //         }
            //         let _ = window.set_focus();
            //     }
            // }
            "shared-files" => {
                if let Err(e) = open_or_create_window(app, WindowLabel::ShareFile) {
                    error!("open shared files window failed: {e}");
                }
            }
            "connect-device" => {
                if let Err(e) = open_or_create_window(app, WindowLabel::ShareFile) {
                    error!("open shared files window for device tab failed: {e}");
                    return;
                }
                if let Some(window) = app.get_window(WindowLabel::ShareFile.label()) {
                    let _ = window.emit("share://open-device-tab", ());
                }
            }
            "app-config" => {
                if let Err(e) = open_or_create_window(app, WindowLabel::Config) {
                    error!("open app config window failed: {e}");
                }
            }
            "toggle-share-server" => {
                let state = app.state::<crate::server::ServerState>();
                match state.is_running() {
                    Ok(true) => {
                        if let Err(e) = state.stop() {
                            error!("stop share server from tray failed: {e}");
                        }
                    }
                    Ok(false) => {
                        let config = app.state::<AppConfigStore>().get();
                        if let Err(e) = state.start(
                            &config.share_server_bind_ip,
                            config.share_server_port,
                            app.clone(),
                        ) {
                            error!("start share server from tray failed: {e}");
                        }
                    }
                    Err(e) => error!("read share server status from tray failed: {e}"),
                }
                update_share_server_menu_label(app);
            }
            "quit" => app.exit(0),
            _ => (),
        })
        .build(app)
        .expect("初始化托盘菜单失败");
}
