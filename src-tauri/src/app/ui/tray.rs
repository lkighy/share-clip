use crate::app::config::AppConfigStore;
use crate::app::ui::window::open_or_create_window;
use crate::models::window::WindowLabel;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Wry};

pub struct TrayMenuState {
    share_server_item: MenuItem<Wry>,
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

pub fn init_menu(app: &App) {
    let clipboard_item = MenuItemBuilder::with_id("index", "剪贴板")
        .build(app)
        .expect("创建菜单项 - 剪贴板失败");
    let shared_files_item = MenuItemBuilder::with_id("shared-files", "分享文件")
        .build(app)
        .expect("创建菜单项 - 分享文件失败");
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
    });

    let menu = MenuBuilder::new(app)
        .items(&[
            &clipboard_item,
            &shared_files_item,
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
                    // TODO: 添加 log
                    println!("{}", e);
                }
            }
            "app-config" => {
                if let Err(e) = open_or_create_window(app, WindowLabel::Config) {
                    // TODO: 添加 log
                    println!("{}", e);
                }
            }
            "toggle-share-server" => {
                let state = app.state::<crate::server::ServerState>();
                match state.is_running() {
                    Ok(true) => {
                        if let Err(e) = state.stop() {
                            println!("{}", e);
                        }
                    }
                    Ok(false) => {
                        let config = app.state::<AppConfigStore>().get();
                        if let Err(e) =
                            state.start(&config.share_server_bind_ip, config.share_server_port)
                        {
                            println!("{}", e);
                        }
                    }
                    Err(e) => println!("{}", e),
                }
                update_share_server_menu_label(app);
            }
            "quit" => app.exit(0),
            _ => (),
        })
        .build(app)
        .expect("初始化托盘菜单失败");
}
