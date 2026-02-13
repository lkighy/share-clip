
/// Windows 平台实现
#[cfg(target_os = "windows")]
pub mod windows {
    use tauri::{Runtime, Window};
    // use winapi::shared::windef::HWND;
    use winapi::um::winuser::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};

    pub fn init_non_activating_window<R: Runtime>(window: &Window<R>) {
        let hwnd = window.hwnd().unwrap().0 as _;

        unsafe {
            // 获取当前窗口扩展样式
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);

            // 设置新的窗口样式 (添加 WS_EX_NOACTIVATE)
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
        }
    }
}

// MacOS 平台实现
#[cfg(target_os = "macos")]
mod macos {
    use tauri::{Runtime, Window};
    use objc::{msg_send, runtime::Object, sel, sel_impl};

    pub fn init_non_activating_panel<R: Runtime>(window: &Window<R>) {
        unsafe {
            let ns_window: *mut Object = window.ns_window().unwrap() as _;

            // 设置为 NSPanel 特性
            let _: () = msg_send![ns_window, setLevel: 3]; // NSFloatingWindowLevel
            let _: () = msg_send![ns_window, setCanBecomeKeyWindow: false];
            let _: () = msg_send![ns_window, setCanBecomeMainWindow: false];
            let _: () = msg_send![ns_window, setCollectionBehavior: 16]; // NSWindowCollectionBehaviorTransient
        }
    }
}
