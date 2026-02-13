
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

#[cfg(target_os = "windows")]
pub mod windows {
    use std::sync::{Mutex, OnceLock};
    use tauri::{AppHandle, Manager, Window};
    use winapi::shared::windef::{HWND, RECT, HHOOK};
    use winapi::um::winuser::{
        CallNextHookEx, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, SetWindowsHookExW,
        UnhookWindowsHookEx, GWL_EXSTYLE, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN,
        WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_XBUTTONDOWN, WS_EX_NOACTIVATE,
    };

    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    static HOOK: Mutex<Option<usize>> = Mutex::new(None);

    /// 初始化无焦点窗口并安装全局鼠标钩子
    pub fn init_non_activating_window(window: &Window) {
        // 1. 设置 WS_EX_NOACTIVATE 样式
        let hwnd = window.hwnd().unwrap().0 as HWND;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
        }

        // 2. 存储 AppHandle（线程安全）
        let app_handle = window.app_handle().clone();
        let _ = APP_HANDLE.set(app_handle);

        // 3. 安装全局鼠标钩子
        install_mouse_hook();
    }

    fn install_mouse_hook() {
        unsafe {
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), std::ptr::null_mut(), 0);
            let mut guard = HOOK.lock().unwrap();
            if !hook.is_null() {
                *guard = Some(hook as usize); // HHOOK → usize
            } else {
                eprintln!("[Tauri] 安装全局鼠标钩子失败");
            }
        }
    }

    unsafe extern "system" fn mouse_proc(n_code: i32, w_param: usize, l_param: isize) -> isize {
        if n_code >= 0 {
            match w_param as u32 {
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => {
                    let msll = &*(l_param as *const MSLLHOOKSTRUCT);
                    let pt = msll.pt;

                    // ---- 第一步：获取窗口句柄，判断点击是否在窗口内 ----
                    let hwnd = if let Some(app_handle) = APP_HANDLE.get() {
                        if let Some(window) = app_handle.get_window("index") {
                            window.hwnd().unwrap().0 as HWND
                        } else {
                            // 窗口不存在，直接返回
                            return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
                        }
                    } else {
                        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
                    };

                    let mut rect: RECT = std::mem::zeroed();
                    GetWindowRect(hwnd, &mut rect);
                    let inside = pt.x >= rect.left && pt.x <= rect.right
                        && pt.y >= rect.top && pt.y <= rect.bottom;

                    if !inside {
                        // ---- 第二步：点击在外部，独立获取 AppHandle 用于隐藏 ----
                        if let Some(app_handle) = APP_HANDLE.get() {
                            let app_handle_clone = app_handle.clone(); // 克隆出独立拥有的 AppHandle
                            let _ = app_handle.run_on_main_thread(move || {
                                // 闭包无参数，直接使用捕获的 app_handle
                                if let Some(window) = app_handle_clone.get_window("index") {
                                    let _ = window.hide();
                                }
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
    }

    /// 卸载鼠标钩子（可选，程序退出时系统会自动清理）
    pub fn uninstall_mouse_hook() {
        unsafe {
            let mut guard = HOOK.lock().unwrap();
            if let Some(hook) = guard.take() {
                UnhookWindowsHookEx(hook as HHOOK); // usize → HHOOK
            }
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
