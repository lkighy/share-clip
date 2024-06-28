use tauri::{WebviewWindow, Window};

pub fn toggle_window_visibility(window: &WebviewWindow) {
    if let Ok(false) = window.is_visible() {
        // window.c
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let _ = window.hide();
    }
}