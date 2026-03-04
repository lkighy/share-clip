#[cfg(target_os = "windows")]
pub mod caret {
    use std::thread;
    use std::time::Duration;

    use tauri::AppHandle;
    use windows::core::{BOOL, Interface};
    use windows::Win32::{
        Foundation::{HWND, POINT, RECT},
        System::Com::{CoCreateInstance, CoInitializeEx, SAFEARRAY, CLSCTX_ALL, COINIT_MULTITHREADED},
        System::Ole::{
            SafeArrayAccessData, SafeArrayGetDim, SafeArrayGetLBound, SafeArrayGetUBound,
            SafeArrayUnaccessData,
        },
        UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern2,
            IUIAutomationTextRange, UIA_TextPattern2Id,
        },
        UI::WindowsAndMessaging::{
            GetCursorPos, GetGUIThreadInfo, GetWindowRect, GUITHREADINFO,
        },
    };

    pub fn get_ui_automation_pos() -> Option<(i32, i32, i32, i32)> {
        for attempt in 0..3 {
            if let Some(rect) = unsafe { get_pos_once() } {
                return Some(rect);
            }
            if attempt < 2 {
                thread::sleep(Duration::from_millis(15));
            }
        }

        unsafe { get_cursor_rect() }
    }

    pub fn get_monitor_bounds_by_point(app: &AppHandle, x: i32, y: i32) -> (i32, i32, i32, i32) {
        app.available_monitors()
            .ok()
            .and_then(|monitors| {
                monitors.into_iter().find_map(|monitor| {
                    let pos = monitor.position();
                    let size = monitor.size();
                    let right = pos.x + size.width as i32;
                    let bottom = pos.y + size.height as i32;

                    (x >= pos.x && x < right && y >= pos.y && y < bottom)
                        .then_some((pos.x, pos.y, right, bottom))
                })
            })
            .or_else(|| {
                app.primary_monitor().ok().flatten().map(|monitor| {
                    let pos = monitor.position();
                    let size = monitor.size();
                    (
                        pos.x,
                        pos.y,
                        pos.x + size.width as i32,
                        pos.y + size.height as i32,
                    )
                })
            })
            .unwrap_or((0, 0, 1920, 1080))
    }

    unsafe fn get_pos_once() -> Option<(i32, i32, i32, i32)> {
        if let Some(rect) = get_gui_thread_caret_rect() {
            return Some(rect);
        }

        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;
        let element = automation.GetFocusedElement().ok()?;

        if let Some(caret_rect) = get_caret_bounding_rect(&element) {
            return Some(caret_rect);
        }

        let rect = element.CurrentBoundingRectangle().ok()?;
        let rect = (rect.left, rect.top, rect.right, rect.bottom);
        if is_valid_rect(rect) {
            return Some(rect);
        }

        None
    }

    unsafe fn get_gui_thread_caret_rect() -> Option<(i32, i32, i32, i32)> {
        let mut info = GUITHREADINFO::default();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;

        if GetGUIThreadInfo(0, &mut info as *mut GUITHREADINFO).is_err() {
            return None;
        }

        if info.hwndCaret.0.is_null() {
            return None;
        }

        caret_rect_to_screen(info.hwndCaret, info.rcCaret)
    }

    unsafe fn caret_rect_to_screen(hwnd: HWND, rect: RECT) -> Option<(i32, i32, i32, i32)> {
        let mut hwnd_rect = RECT::default();
        if GetWindowRect(hwnd, &mut hwnd_rect as *mut RECT).is_err() {
            return None;
        }
        let out = (
            hwnd_rect.left + rect.left,
            hwnd_rect.top + rect.top,
            hwnd_rect.left + rect.right,
            hwnd_rect.top + rect.bottom,
        );
        if is_valid_rect(out) {
            Some(out)
        } else {
            None
        }
    }

    unsafe fn get_cursor_rect() -> Option<(i32, i32, i32, i32)> {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt as *mut POINT).is_err() {
            return None;
        }
        Some((pt.x, pt.y, pt.x + 1, pt.y + 1))
    }

    unsafe fn get_caret_bounding_rect(
        element: &IUIAutomationElement,
    ) -> Option<(i32, i32, i32, i32)> {
        if let Ok(pattern) = element.GetCurrentPattern(UIA_TextPattern2Id) {
            if let Ok(text_pattern2) = pattern.cast::<IUIAutomationTextPattern2>() {
                let mut is_active = BOOL(0);
                let range = text_pattern2.GetCaretRange(&mut is_active as *mut BOOL).ok()?;
                if is_active.as_bool() {
                    if let Some(rect) = get_range_bounding_rect(&range) {
                        return Some(rect);
                    }
                }
            }
        }

        // `GetVisibleRanges` may hang in Chromium-based apps when selection/click state churns quickly.
        // Skip this fallback to keep caret lookup non-blocking and rely on element/cursor fallbacks instead.

        None
    }

    unsafe fn get_range_bounding_rect(
        range: &IUIAutomationTextRange,
    ) -> Option<(i32, i32, i32, i32)> {
        let sa_ptr = range.GetBoundingRectangles().ok()?;
        if sa_ptr.is_null() {
            return None;
        }

        let sa = sa_ptr as *const SAFEARRAY;
        if (*sa).cbElements != std::mem::size_of::<f64>() as u32 {
            return None;
        }
        if SafeArrayGetDim(sa_ptr) != 1 {
            return None;
        }

        let lower = SafeArrayGetLBound(sa_ptr, 1).ok()?;
        let upper = SafeArrayGetUBound(sa_ptr, 1).ok()?;
        let count = upper - lower + 1;
        if count < 4 {
            return None;
        }

        let mut p_data: *mut f64 = std::ptr::null_mut();
        SafeArrayAccessData(sa_ptr, &mut p_data as *mut _ as _).ok()?;

        let left = *p_data.add(0);
        let top = *p_data.add(1);
        let width = *p_data.add(2);
        let height = *p_data.add(3);

        let _ = SafeArrayUnaccessData(sa_ptr);

        let out = (
            left.round() as i32,
            top.round() as i32,
            (left + width).round() as i32,
            (top + height).round() as i32,
        );

        if is_valid_rect(out) {
            Some(out)
        } else {
            None
        }
    }

    fn is_valid_rect((left, top, right, bottom): (i32, i32, i32, i32)) -> bool {
        right > left && bottom > top
    }
}
