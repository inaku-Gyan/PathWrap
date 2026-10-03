//! 悬浮窗的原生窗口控制：扩展样式、显隐、以物理像素停靠。
//!
//! 设计要点：
//! - 悬浮窗保留工具窗口和置顶属性，但允许正常激活，使 egui `TextEdit` 能获得真实
//!   Windows 焦点并接收系统输入、选区、剪贴板和输入法事件。
//! - 应用扩展样式后带 `SWP_FRAMECHANGED` 刷新，确保样式立即生效；同时明确清除旧的
//!   `WS_EX_NOACTIVATE`，避免升级运行时仍沿用旧窗口样式。
//! - “隐藏”用移到屏幕外实现（保持 `WS_VISIBLE`，**不** `SW_HIDE`）：被 `SW_HIDE`
//!   的窗口收不到绘制/唤醒，会饿死 eframe 事件循环；停到屏幕外则事件循环长活。
//! - 定位一律使用物理像素，直接匹配对话框的 DWM 视觉边界，避免贴边缝隙。

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

/// 隐藏时把窗口挪到的屏幕外坐标（与系统最小化窗口所用坐标一致，安全越界）。
const OFFSCREEN: i32 = -32000;
fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut core::ffi::c_void)
}

/// 应用悬浮窗扩展样式：可正常激活 + 工具窗口（不进任务栏）+ 置顶。
///
/// 幂等：重复调用无副作用；同时清除旧版本遗留的 `WS_EX_NOACTIVATE`。返回是否成功
/// 应用（`hwnd` 为 0 时返回 false）。
pub fn apply_overlay_ex_styles(handle: isize) -> bool {
    if handle == 0 {
        return false;
    }
    let target = hwnd(handle);
    let desired = (WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;

    unsafe {
        let current = GetWindowLongPtrW(target, GWL_EXSTYLE);
        let updated = (current & !(WS_EX_NOACTIVATE.0 as isize)) | desired;
        if current == updated {
            return true;
        }
        SetWindowLongPtrW(target, GWL_EXSTYLE, updated);
        // 带 SWP_FRAMECHANGED，让扩展样式立即生效；更新样式本身不激活、不移动、
        // 不改尺寸。
        let _ = SetWindowPos(
            target,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    true
}

/// 以物理像素停靠悬浮窗到指定矩形，并在不激活的情况下显示。
pub fn dock(handle: isize, x: i32, y: i32, width: i32, height: i32) {
    if handle == 0 {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            hwnd(handle),
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
        );
    }
}

/// “隐藏”悬浮窗：仅移到屏幕外，保持窗口可见状态（不 `SW_HIDE`）。
///
/// 关键区别：被 `SW_HIDE` 的窗口收不到 `WM_PAINT`/唤醒，会饿死由绘制驱动的 eframe
/// 事件循环，导致再也无法响应后续对话框；停到屏幕外则窗口长活、随时可被重新停靠。
pub fn park(handle: isize) {
    if handle == 0 {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            hwnd(handle),
            None,
            OFFSCREEN,
            OFFSCREEN,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WINDOW_EX_STYLE, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{PCWSTR, w};

    unsafe extern "system" fn test_wndproc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(h, msg, w, l) }
    }

    /// 建一个隐藏测试窗口（注册类幂等；失败时类已存在，忽略）。
    fn create_hidden_window() -> isize {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let class = w!("PathWarpWindowExTest");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(test_wndproc),
                hInstance: hinstance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            let _ = RegisterClassW(&wc); // 同类重复注册返回 0，无害。
            let handle = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                PCWSTR::null(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                100,
                100,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("create test window");
            handle.0 as isize
        }
    }

    /// 回归核心：旧版本的非激活扩展样式必须被清除，悬浮窗才能获得真实 Windows 焦点。
    #[test]
    fn overlay_allows_real_activation() {
        let handle = create_hidden_window();

        assert!(apply_overlay_ex_styles(handle));
        let ex_style = unsafe { GetWindowLongPtrW(hwnd(handle), GWL_EXSTYLE) };
        assert!(
            ex_style & (WS_EX_NOACTIVATE.0 as isize) == 0,
            "WS_EX_NOACTIVATE must be cleared after apply_overlay_ex_styles"
        );

        unsafe {
            let _ = DestroyWindow(hwnd(handle));
        }
    }

    #[test]
    fn zero_handle_is_a_noop() {
        assert!(!apply_overlay_ex_styles(0));
        dock(0, 1, 2, 3, 4);
        park(0);
    }
}
