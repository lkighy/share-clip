use crate::app::config::AppConfigStore;
use crate::models::window::WindowLabel;
#[cfg(target_os = "windows")]
use crate::platform::non_activating::windows;
use tauri::{App, LogicalSize, Manager, WebviewUrl, WebviewWindow};

pub fn init_app(app: &mut App) {
    let config = app.state::<AppConfigStore>();
    let config = config.get();

    if let Some(window) = app.get_window("index") {
        let _ = window.set_size(LogicalSize::new(
            config.clipboard_window_width.max(200) as f64,
            config.clipboard_window_height.max(120) as f64,
        ));

        #[cfg(target_os = "windows")]
        windows::init_non_activating_window(&window);
    };
}

pub fn open_or_create_window(app: &tauri::AppHandle, label: WindowLabel) -> Result<(), String> {
    if let Some(window) = app.get_window(label.label()) {
        if let Ok(false) = window.is_visible() {
            window.show().map_err(|e| e.to_string())?;
        }
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // let window = WindowBuilder::from_config(app, &config)
    //     .map_err(|e| e.to_string())?
    //     .build().map_err(|e| e.to_string())?;

    // window.de
    // window.op

    WebviewWindow::builder(
        app,
        label.label(),
        WebviewUrl::App(label.url_params().into()),
    )
    .title(label.title())
    .inner_size(980.0, 700.0)
    .skip_taskbar(matches!(label, WindowLabel::Clipboard))
    .decorations(false)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}
