//! E2E 测试用的真实文件对话框宿主。
//!
//! 通过 stdin 行协议控制（由 `tests/e2e.rs` 驱动）：
//! - `open`  打开一个真实的 `IFileOpenDialog`（在后台 STA 线程阻塞显示）
//! - `close` 关闭当前对话框（向本进程的 `#32770` 发 `WM_CLOSE`）
//! - `probe` 打开一个非文件对话框窗口，标题计数收到的 G 按键
//! - `lock` / `unlock` 测试前台进程拒绝/恢复普通激活
//! - `exit`  退出进程
//!
//! stdout 仅输出 `READY` 一行用于启动同步；对话框的出现/消失由测试进程
//! 直接用 Win32 轮询判定，避免双向协议的时序歧义。
#![allow(
    clippy::print_stdout,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::io::BufRead;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_G;
use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
    GWLP_USERDATA, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowLongPtrW,
    GetWindowThreadProcessId, LSFW_LOCK, LSFW_UNLOCK, LockSetForegroundWindow, MSG, PostMessageW,
    PostQuitMessage, RegisterClassW, SetWindowLongPtrW, SetWindowTextW, TranslateMessage,
    WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{BOOL, PCWSTR, w};

fn main() {
    // Exercise real per-monitor dialog DPI when the desktop checks move this host.
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    println!("READY");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        match line.trim() {
            "open" => {
                std::thread::spawn(|| {
                    unsafe {
                        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                        let dialog = CoCreateInstance::<_, IFileOpenDialog>(
                            &FileOpenDialog,
                            None,
                            CLSCTX_INPROC_SERVER,
                        );
                        if let Ok(dialog) = dialog {
                            // 用户取消/被 WM_CLOSE 关闭都会让 Show 返回 Err，属预期。
                            let _ = dialog.Show(None);
                        }
                        if hr.is_ok() {
                            CoUninitialize();
                        }
                    }
                });
            }
            "close" => close_own_dialogs(),
            "probe" => {
                std::thread::spawn(shortcut_probe);
            }
            "lock" => set_focus_lock(true),
            "unlock" => set_focus_lock(false),
            "exit" => break,
            _ => {}
        }
    }
}

fn set_focus_lock(locked: bool) {
    unsafe {
        let foreground = GetForegroundWindow();
        let mut pid = 0;
        GetWindowThreadProcessId(foreground, Some(&mut pid));
        if pid != GetCurrentProcessId() {
            return;
        }
        if LockSetForegroundWindow(if locked { LSFW_LOCK } else { LSFW_UNLOCK }).is_ok() {
            let title = if locked {
                w!("PathWarp focus lock: locked")
            } else {
                w!("PathWarp focus lock: unlocked")
            };
            let _ = SetWindowTextW(foreground, title);
        }
    }
}

fn shortcut_probe() {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match message {
                WM_KEYDOWN if wparam.0 == usize::from(VK_G.0) => {
                    let count = GetWindowLongPtrW(hwnd, GWLP_USERDATA) + 1;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, count);
                    let title: Vec<u16> = format!("PathWarp shortcut probe: {count}\0")
                        .encode_utf16()
                        .collect();
                    let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
                    LRESULT(0)
                }
                WM_CLOSE => {
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, message, wparam, lparam),
            }
        }
    }
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let class = w!("PathWarpShortcutProbe");
        let instance = module.into();
        let _ = RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        });
        let Ok(_window) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("PathWarp shortcut probe: 0"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            200,
            None,
            None,
            Some(instance),
            None,
        ) else {
            return;
        };
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// 向本进程所有 `#32770` 顶层窗口发送 `WM_CLOSE`。
fn close_own_dialogs() {
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let own_pid = lparam.0 as u32;
        let mut pid = 0u32;
        unsafe {
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == own_pid {
            let mut class_buf = [0u16; 64];
            let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
            if let Ok(n) = usize::try_from(len)
                && String::from_utf16_lossy(&class_buf[..n]) == "#32770"
            {
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
        }
        BOOL(1)
    }

    let pid = unsafe { GetCurrentProcessId() };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(pid as isize));
    }
}
