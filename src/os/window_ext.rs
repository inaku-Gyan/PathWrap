//! 悬浮窗的原生窗口控制：非激活扩展样式、子类化拦截激活、显隐、以物理像素停靠。
//!
//! 设计要点：
//! - 悬浮窗设为 `WS_EX_NOACTIVATE`，永不抢占前台焦点，因此目标文件对话框在用户
//!   与悬浮窗交互期间始终保持前台——这从根源上消除了整套“焦点交接”显隐补丁。
//! - 应用扩展样式后带 `SWP_FRAMECHANGED` 刷新，确保样式立即在命中测试/激活判定
//!   里生效（否则点击可能仍激活悬浮窗）。
//! - 再子类化窗口过程，对 `WM_MOUSEACTIVATE` 硬性返回 `MA_NOACTIVATE`，作为“点击
//!   永不激活”的第二重保证（Listary 同款手法）。
//! - “隐藏”用移到屏幕外实现（保持 `WS_VISIBLE`，**不** `SW_HIDE`）：被 `SW_HIDE`
//!   的窗口收不到绘制/唤醒，会饿死 eframe 事件循环；停到屏幕外则事件循环长活。
//! - 定位一律使用物理像素，直接匹配对话框的 DWM 视觉边界，避免贴边缝隙。

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, MA_NOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW,
    SetWindowPos, WM_MOUSEACTIVATE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

/// 隐藏时把窗口挪到的屏幕外坐标（与系统最小化窗口所用坐标一致，安全越界）。
const OFFSCREEN: i32 = -32000;
/// 子类化标识（同一 SUBCLASSPROC 下用于区分不同订阅者）。
const SUBCLASS_ID: usize = 0x5057_0001;

fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut core::ffi::c_void)
}

/// 应用悬浮窗扩展样式：非激活 + 工具窗口（不进任务栏）+ 置顶。
///
/// 幂等：重复调用无副作用。返回是否成功应用（`hwnd` 为 0 时返回 false）。
pub fn apply_overlay_ex_styles(handle: isize) -> bool {
    if handle == 0 {
        return false;
    }
    let target = hwnd(handle);
    let desired = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;

    unsafe {
        let current = GetWindowLongPtrW(target, GWL_EXSTYLE);
        if current & desired == desired {
            return true;
        }
        SetWindowLongPtrW(target, GWL_EXSTYLE, current | desired);
        // 关键：带 SWP_FRAMECHANGED，让扩展样式立即生效（且不激活、不移动、不改尺寸）。
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

/// 子类化窗口过程，对 `WM_MOUSEACTIVATE` 返回 `MA_NOACTIVATE`：点击悬浮窗永不激活它。
/// 返回是否成功安装。
pub fn install_noactivate_subclass(handle: isize) -> bool {
    if handle == 0 {
        return false;
    }
    unsafe { SetWindowSubclass(hwnd(handle), Some(subclass_proc), SUBCLASS_ID, 0).as_bool() }
}

unsafe extern "system" fn subclass_proc(
    handle: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        // 收到点击命中：告诉系统“不要激活我”，前台留在对话框，鼠标消息照常处理。
        return LRESULT(MA_NOACTIVATE as isize);
    }
    unsafe { DefSubclassProc(handle, msg, wparam, lparam) }
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
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SendMessageW,
        WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
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

    /// 回归核心：加了非激活样式 + 子类化后，`WM_MOUSEACTIVATE` 必须返回 `MA_NOACTIVATE`
    /// （即点击悬浮窗永不激活它——用户报告的“点击即消失”的根因修复）。
    #[test]
    fn overlay_declines_mouse_activation() {
        let handle = create_hidden_window();

        assert!(apply_overlay_ex_styles(handle));
        let ex_style = unsafe { GetWindowLongPtrW(hwnd(handle), GWL_EXSTYLE) };
        assert!(
            ex_style & (WS_EX_NOACTIVATE.0 as isize) != 0,
            "WS_EX_NOACTIVATE must be present after apply_overlay_ex_styles"
        );

        install_noactivate_subclass(handle);
        let result = unsafe { SendMessageW(hwnd(handle), WM_MOUSEACTIVATE, None, None) };
        assert_eq!(
            result.0, MA_NOACTIVATE as isize,
            "subclass must answer WM_MOUSEACTIVATE with MA_NOACTIVATE"
        );

        unsafe {
            let _ = DestroyWindow(hwnd(handle));
        }
    }

    #[test]
    fn zero_handle_is_a_noop() {
        assert!(!apply_overlay_ex_styles(0));
        install_noactivate_subclass(0);
        dock(0, 1, 2, 3, 4);
        park(0);
    }
}
