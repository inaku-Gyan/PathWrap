use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, SendMessageW, SetForegroundWindow, WM_KEYDOWN, WM_KEYUP, WM_SETTEXT,
};

const WM_USER: u32 = 0x0400;
const CDM_FIRST: u32 = WM_USER + 100;
const CDM_SETCONTROLTEXT: u32 = CDM_FIRST + 0x0004;
const EDT1: usize = 0x0480;
const VK_RETURN_WPARAM: usize = 0x0D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendMessageAction {
    CdmSetControlText,
    EditSetTextAndEnter,
}

pub fn inject_folder_path(dialog_hwnd: isize, target_path: &str) -> Result<SendMessageAction, String> {
    if dialog_hwnd == 0 {
        return Err("invalid dialog hwnd".to_string());
    }
    if target_path.trim().is_empty() {
        return Err("target path is empty".to_string());
    }

    let dialog = HWND(dialog_hwnd);

    unsafe {
        let _ = SetForegroundWindow(dialog);
    }

    if try_cdm_setcontroltext(dialog, target_path) {
        return Ok(SendMessageAction::CdmSetControlText);
    }

    if try_edit_settext_enter(dialog, target_path) {
        return Ok(SendMessageAction::EditSetTextAndEnter);
    }

    Err("failed to inject path into file dialog".to_string())
}

fn try_cdm_setcontroltext(dialog: HWND, target_path: &str) -> bool {
    let text = to_wide_null(target_path);

    unsafe {
        let _ = SendMessageW(
            dialog,
            CDM_SETCONTROLTEXT,
            WPARAM(EDT1),
            LPARAM(text.as_ptr() as isize),
        );
    }

    send_enter(dialog)
}

fn try_edit_settext_enter(dialog: HWND, target_path: &str) -> bool {
    let edit = find_filename_edit(dialog).unwrap_or(dialog);
    let text = to_wide_null(target_path);

    unsafe {
        let _ = SendMessageW(
            edit,
            WM_SETTEXT,
            WPARAM(0),
            LPARAM(text.as_ptr() as isize),
        );
    }

    send_enter(edit)
}

fn find_filename_edit(dialog: HWND) -> Option<HWND> {
    unsafe {
        let combo_ex = FindWindowExW(dialog, HWND(0), w!("ComboBoxEx32"), PCWSTR::null());
        if combo_ex.0 != 0 {
            let combo = FindWindowExW(combo_ex, HWND(0), w!("ComboBox"), PCWSTR::null());
            if combo.0 != 0 {
                let edit = FindWindowExW(combo, HWND(0), w!("Edit"), PCWSTR::null());
                if edit.0 != 0 {
                    return Some(edit);
                }
            }
        }

        let direct_edit = FindWindowExW(dialog, HWND(0), w!("Edit"), PCWSTR::null());
        if direct_edit.0 != 0 {
            Some(direct_edit)
        } else {
            None
        }
    }
}

fn send_enter(target: HWND) -> bool {
    unsafe {
        let _ = SendMessageW(
            target,
            WM_KEYDOWN,
            WPARAM(VK_RETURN_WPARAM),
            LPARAM(0),
        );
        let _ = SendMessageW(
            target,
            WM_KEYUP,
            WPARAM(VK_RETURN_WPARAM),
            LPARAM(0),
        );
    }

    true
}

fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
