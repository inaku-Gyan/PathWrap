use std::mem::size_of;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextW,
    IsWindow, IsWindowVisible,
};
use windows::core::w;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogInfo {
    pub hwnd: isize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
}

pub fn start_monitor(sender: Sender<Option<DialogInfo>>, ctx: egui::Context) {
    const INVALID_HWND: isize = 0;
    const IDLE_POLL_INTERVAL_MS: u64 = 30;
    const TRACKING_POLL_INTERVAL_MS: u64 = 8;
    const LOST_CONFIRM_TICKS: u8 = 3;

    let mut last_hwnd: isize = INVALID_HWND;
    let mut last_foreground_signature: Option<String> = None;
    let mut lost_ticks: u8 = 0;

    loop {
        let loop_started = Instant::now();
        let current_dialog = get_active_file_dialog();

        if let Some(info) = current_dialog {
            lost_ticks = 0;
            if last_hwnd != info.hwnd {
                log::debug!(
                    "[monitor] dialog detected: hwnd={} rect=({}, {}) {}x{}",
                    info.hwnd,
                    info.x,
                    info.y,
                    info.width,
                    info.height
                );
                last_hwnd = info.hwnd;
            }
            let _ = sender.send(Some(info));
            ctx.request_repaint();
        } else if last_hwnd != INVALID_HWND {
            // Keep following only the previously-accepted dialog to survive short focus jumps
            // without re-opening detection on unrelated top-level windows.
            if let Some(info) = get_dialog_info_by_hwnd(last_hwnd) {
                lost_ticks = 0;
                let _ = sender.send(Some(info));
                ctx.request_repaint();
            } else {
                // Handle common dialog-handle recreation during open/save transitions.
                // This fallback only runs while we already have a trusted last_hwnd.
                if let Some(info) = find_any_file_dialog() {
                    if last_hwnd != info.hwnd {
                        log::debug!("[monitor] dialog switched: {} -> {}", last_hwnd, info.hwnd);
                        last_hwnd = info.hwnd;
                    }
                    lost_ticks = 0;
                    let _ = sender.send(Some(info));
                    ctx.request_repaint();
                } else {
                    lost_ticks = lost_ticks.saturating_add(1);
                    if lost_ticks >= LOST_CONFIRM_TICKS {
                        log::debug!("[monitor] dialog lost: hwnd={}", last_hwnd);
                        last_hwnd = INVALID_HWND;
                        lost_ticks = 0;
                        let _ = sender.send(None);
                        ctx.request_repaint();
                    }
                }
            }
        } else if let Some(sig) = get_foreground_signature()
            && last_foreground_signature.as_deref() != Some(sig.as_str())
        {
            log::trace!("[monitor] foreground: {}", sig);
            last_foreground_signature = Some(sig);
        }

        let poll_interval = if last_hwnd != INVALID_HWND {
            TRACKING_POLL_INTERVAL_MS
        } else {
            IDLE_POLL_INTERVAL_MS
        };
        let target_interval = Duration::from_millis(poll_interval);
        let elapsed = loop_started.elapsed();
        if elapsed < target_interval {
            std::thread::sleep(target_interval - elapsed);
        }
    }
}

pub fn get_active_file_dialog() -> Option<DialogInfo> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0 != 0
        && let Some(info) = get_dialog_info_if_match(hwnd)
    {
        return Some(info);
    }

    // Intentionally no global scan here: new detection must come from foreground to reduce
    // false positives from generic #32770 system dialogs (e.g. warning/message boxes).
    None
}

pub fn is_foreground_hwnd(hwnd: isize) -> bool {
    unsafe { GetForegroundWindow().0 == hwnd }
}

fn get_dialog_info_if_match(hwnd: HWND) -> Option<DialogInfo> {
    if unsafe { !IsWindowVisible(hwnd).as_bool() } {
        return None;
    }

    let class_string = get_class_name(hwnd);
    if class_string != "#32770" {
        return None;
    }

    let title = get_window_text(hwnd);
    let title_lower = title.to_lowercase();

    let title_looks_like_file_dialog = title.contains("打开")
        || title.contains("保存")
        || title.contains("另存为")
        || title.contains("选择")
        || title_lower.contains("open")
        || title_lower.contains("save")
        || title_lower.contains("select");

    // File dialogs and generic alert dialogs both use #32770. To distinguish them, require
    // structure that looks like a file browser surface, not just a matching title.
    let has_combo = has_child_class(hwnd, w!("ComboBoxEx32"));
    let has_directui = has_child_class(hwnd, w!("DirectUIHWND"));
    let has_shell_view = has_child_class(hwnd, w!("SHELLDLL_DefView"));
    let has_dui_view = has_child_class(hwnd, w!("DUIViewWndClassName"));

    // Require stronger structural evidence to avoid matching generic #32770 alerts.
    let has_strong_structure = (has_combo && (has_directui || has_shell_view || has_dui_view))
        || (has_directui && has_shell_view)
        || (has_directui && has_dui_view);

    let has_file_dialog_signature = has_strong_structure
        || (title_looks_like_file_dialog
            && (has_combo || has_directui || has_shell_view || has_dui_view));

    if has_file_dialog_signature {
        get_dialog_info(hwnd)
    } else {
        None
    }
}

fn has_child_class(parent: HWND, class_name: windows::core::PCWSTR) -> bool {
    unsafe { FindWindowExW(parent, HWND(0), class_name, windows::core::PCWSTR::null()).0 != 0 }
}

fn find_any_file_dialog() -> Option<DialogInfo> {
    let mut hwnds: Vec<isize> = Vec::new();

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let hwnds = unsafe { &mut *(lparam.0 as *mut Vec<isize>) };
        hwnds.push(hwnd.0);
        BOOL(1)
    }

    let lparam = LPARAM((&mut hwnds as *mut Vec<isize>) as isize);
    let _ = unsafe { EnumWindows(Some(enum_proc), lparam) };

    for hwnd_raw in hwnds {
        let hwnd = HWND(hwnd_raw as _);
        if let Some(info) = get_dialog_info_if_match(hwnd) {
            return Some(info);
        }
    }

    None
}

fn get_class_name(hwnd: HWND) -> String {
    unsafe {
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            String::from_utf16_lossy(&class_name[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let mut text = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut text);
        if len > 0 {
            String::from_utf16_lossy(&text[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_foreground_signature() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return None;
        }

        let class_name = get_class_name(hwnd);
        let title = get_window_text(hwnd);
        Some(format!(
            "hwnd={} class='{}' title='{}'",
            hwnd.0, class_name, title
        ))
    }
}

pub fn get_dialog_info_by_hwnd(hwnd_isize: isize) -> Option<DialogInfo> {
    let hwnd = HWND(hwnd_isize as _);
    if unsafe { IsWindow(hwnd).as_bool() && IsWindowVisible(hwnd).as_bool() } {
        get_dialog_info(hwnd)
    } else {
        None
    }
}

fn get_dialog_info(hwnd: HWND) -> Option<DialogInfo> {
    if let Some(rect) = get_window_visual_rect(hwnd) {
        let dpi = get_window_dpi(hwnd);
        Some(DialogInfo {
            hwnd: hwnd.0,
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            dpi,
        })
    } else {
        None
    }
}

fn get_window_visual_rect(hwnd: HWND) -> Option<RECT> {
    let mut visual_rect = RECT::default();
    let dwm_result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut visual_rect as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    };

    if dwm_result.is_ok() {
        return Some(visual_rect);
    }

    let mut window_rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut window_rect) }.is_ok() {
        Some(window_rect)
    } else {
        None
    }
}

fn get_window_dpi(hwnd: HWND) -> u32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 96 } else { dpi }
}
