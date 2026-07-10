//! 悬浮窗的原生窗口控制：非激活扩展样式、显隐、以物理像素停靠。
//!
//! 设计要点：悬浮窗设为 `WS_EX_NOACTIVATE`，永不抢占前台焦点，因此目标文件
//! 对话框在用户与悬浮窗交互期间始终保持前台——这从根源上消除了旧实现里
//! 一整套“焦点交接”显隐补丁。定位一律使用物理像素，直接匹配对话框的 DWM
//! 视觉边界，避免逻辑点 / DPI 口径不一致导致的贴边缝隙。

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

/// 隐藏时把窗口挪到的屏幕外坐标（与系统最小化窗口所用坐标一致，安全越界）。
const OFFSCREEN: i32 = -32000;

fn hwnd(handle: isize) -> HWND {
    HWND(handle as _)
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
        // 让样式变更立即生效，且不激活窗口。
        let _ = SetWindowPos(
            target,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
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
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
        );
    }
}

/// 隐藏悬浮窗。
///
/// 先移到屏幕外再 `SW_HIDE`：启动阶段 winit/eframe 可能在我们首帧隐藏之后又把
/// 窗口显示出来（默认位置），单靠 `SW_HIDE` 会输掉这个竞争而残留一个矩形；移到
/// 屏幕外后，即便被重新显示也落在可见区域之外，不会出现“关不掉的黑色矩形”。
pub fn hide(handle: isize) {
    if handle == 0 {
        return;
    }
    let target = hwnd(handle);
    unsafe {
        let _ = SetWindowPos(
            target,
            HWND_TOPMOST,
            OFFSCREEN,
            OFFSCREEN,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(target, SW_HIDE);
    }
}
