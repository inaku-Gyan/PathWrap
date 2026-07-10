//! E2E 测试用的真实文件对话框宿主。
//!
//! 通过 stdin 行协议控制（由 `tests/e2e.rs` 驱动）：
//! - `open`  打开一个真实的 `IFileOpenDialog`（在后台 STA 线程阻塞显示）
//! - `close` 关闭当前对话框（向本进程的 `#32770` 发 `WM_CLOSE`）
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
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};
use windows::core::BOOL;

fn main() {
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
            "exit" => break,
            _ => {}
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
