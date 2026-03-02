
// /// Windows 平台实现
// #[cfg(target_os = "windows")]
// pub mod windows {
//     use tauri::{Runtime, Window};
//     // use winapi::shared::windef::HWND;
//     use winapi::um::winuser::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};
//
//     pub fn init_non_activating_window<R: Runtime>(window: &Window<R>) {
//         let hwnd = window.hwnd().unwrap().0 as _;
//
//         unsafe {
//             // 获取当前窗口扩展样式
//             let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
//
//             // 设置新的窗口样式 (添加 WS_EX_NOACTIVATE)
//             SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
//         }
//     }
// }

// TODO: 存在致命Bug， 在chrome浏览器中如果选中文本时，唤出剪切板会导致chrome卡死
#[cfg(target_os = "windows")]
pub mod windows {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    use std::thread;
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, Manager, Window};
    use winapi::shared::windef::{HWND, POINT, RECT};
    use winapi::um::winuser::{
        GetAsyncKeyState, GetCursorPos, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW,
        IsWindowVisible, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOWNA, VK_LBUTTON, VK_MBUTTON,
        VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2, WS_EX_NOACTIVATE,
    };

    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    static WATCHER_STARTED: AtomicBool = AtomicBool::new(false);

    /// 初始化无焦点窗口并启动自动隐藏检测（轮询，不使用全局低级鼠标钩子）
    pub fn init_non_activating_window(window: &Window) {
        let hwnd = window.hwnd().unwrap().0 as HWND;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
        }

        let app_handle = window.app_handle().clone();
        let _ = APP_HANDLE.set(app_handle);
        start_auto_hide_watcher();
    }

    /// 以不激活的方式显示窗口
    pub fn show_window_non_activating(window: &Window) {
        let hwnd = window.hwnd().unwrap().0 as HWND;
        unsafe {
            ShowWindow(hwnd, SW_SHOWNA);
        }

        let _ = window.emit("clipboard-window-invoked", ());
    }

    fn start_auto_hide_watcher() {
        if WATCHER_STARTED.swap(true, Ordering::AcqRel) {
            return;
        }

        thread::spawn(|| {
            let mut hide_latched = false;

            loop {
                thread::sleep(Duration::from_millis(25));

                let down = is_any_mouse_button_down();
                let Some(app_handle) = APP_HANDLE.get() else {
                    hide_latched = false;
                    continue;
                };

                let Some(window) = app_handle.get_window("index") else {
                    hide_latched = false;
                    continue;
                };

                let hwnd = window.hwnd().unwrap().0 as HWND;
                if unsafe { IsWindowVisible(hwnd) } == 0 {
                    hide_latched = false;
                    continue;
                }

                if down && !hide_latched {
                    if !is_cursor_inside_window(hwnd) {
                        unsafe {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                        hide_latched = true;
                    }
                }

                if !down {
                    hide_latched = false;
                }
            }
        });
    }

    fn is_any_mouse_button_down() -> bool {
        unsafe {
            (GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RBUTTON) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_MBUTTON) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_XBUTTON1) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_XBUTTON2) as u16 & 0x8000) != 0
        }
    }

    fn is_cursor_inside_window(hwnd: HWND) -> bool {
        unsafe {
            let mut pt: POINT = std::mem::zeroed();
            if GetCursorPos(&mut pt) == 0 {
                return false;
            }

            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return false;
            }

            pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom
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
