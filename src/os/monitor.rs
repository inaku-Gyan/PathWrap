use std::sync::mpsc::Sender;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowRect, IsWindow, IsWindowVisible,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogInfo {
    pub hwnd: isize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn start_monitor(sender: Sender<Option<DialogInfo>>, ctx: egui::Context) {
    let mut last_hwnd: isize = 0;

    loop {
        std::thread::sleep(Duration::from_millis(100));

        let current_dialog = get_active_file_dialog();

        if let Some(info) = current_dialog {
            if last_hwnd != info.hwnd {
                last_hwnd = info.hwnd;
                // Found a new dialog, tell the app to wake up and position itself
                let _ = sender.send(Some(info));
                ctx.request_repaint();
            } else {
                // If it's the same dialog, maybe its position changed, update it too
                // (optional: can compare x,y,w,h to avoid spam)
                let _ = sender.send(Some(info));
                ctx.request_repaint();
            }
        } else {
            // Check if the previously locked dialog is closed or user clicked away
            if last_hwnd != 0 {
                // If the foreground window isn't a dialog, but our tracked dialog is still open,
                // we might want to keep our UI or hide it.
                // For a highly integrated feel, let's keep tracking the locked dialog if it's still alive.
                if let Some(info) = get_dialog_info_by_hwnd(last_hwnd) {
                    let _ = sender.send(Some(info));
                    ctx.request_repaint();
                } else {
                    // Dialog actually closed
                    last_hwnd = 0;
                    let _ = sender.send(None);
                    ctx.request_repaint();
                }
            }
        }
    }
}

pub fn get_active_file_dialog() -> Option<DialogInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return None;
        }

        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len == 0 {
            return None;
        }

        let class_string = String::from_utf16_lossy(&class_name[..len as usize]);

        // #32770 is the standard dialog class name
        if class_string == "#32770" {
            get_dialog_info(hwnd)
        } else {
            None
        }
    }
}

pub fn get_dialog_info_by_hwnd(hwnd_isize: isize) -> Option<DialogInfo> {
    unsafe {
        let hwnd = HWND(hwnd_isize as _);
        if IsWindow(hwnd).as_bool() && IsWindowVisible(hwnd).as_bool() {
            get_dialog_info(hwnd)
        } else {
            None
        }
    }
}

unsafe fn get_dialog_info(hwnd: HWND) -> Option<DialogInfo> {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
        Some(DialogInfo {
            hwnd: hwnd.0,
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    } else {
        None
    }
}
