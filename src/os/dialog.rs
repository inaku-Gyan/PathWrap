use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowTextLengthW, GetWindowTextW, SendMessageW, SetForegroundWindow,
    WM_COMMAND, WM_KEYDOWN, WM_KEYUP, WM_SETTEXT,
};
use windows::core::{PCWSTR, w};

const WM_USER: u32 = 0x0400;
const CDM_FIRST: u32 = WM_USER + 100;
const CDM_SETCONTROLTEXT: u32 = CDM_FIRST + 0x0004;
const EDT1: usize = 0x0480;
const IDOK_WPARAM: usize = 1;
const VK_ALT: u16 = 0x12;
const VK_BACK: u16 = 0x08;
const VK_CONTROL: u16 = 0x11;
const VK_A: u16 = 0x41;
const VK_D: u16 = 0x44;
const VK_RETURN_WPARAM: usize = 0x0D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendMessageAction {
    CdmSetControlText,
    EditSetTextAndEnter,
    KeyboardNavigate,
}

pub fn inject_folder_path(
    dialog_hwnd: isize,
    target_path: &str,
) -> Result<SendMessageAction, String> {
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

    if try_keyboard_navigate(dialog, target_path) {
        return Ok(SendMessageAction::KeyboardNavigate);
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

    if let Some(edit) = find_filename_edit(dialog)
        && text_matches(edit, target_path)
    {
        trigger_confirm(dialog, edit)
    } else {
        false
    }
}

fn try_edit_settext_enter(dialog: HWND, target_path: &str) -> bool {
    let Some(edit) = find_filename_edit(dialog) else {
        return false;
    };
    let text = to_wide_null(target_path);

    unsafe {
        let _ = SendMessageW(edit, WM_SETTEXT, WPARAM(0), LPARAM(text.as_ptr() as isize));
    }

    if text_matches(edit, target_path) {
        trigger_confirm(dialog, edit)
    } else {
        false
    }
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

fn trigger_confirm(dialog: HWND, edit: HWND) -> bool {
    send_enter(edit);
    send_enter(dialog);

    unsafe {
        let _ = SendMessageW(dialog, WM_COMMAND, WPARAM(IDOK_WPARAM), LPARAM(0));
    }

    true
}

fn send_enter(target: HWND) {
    unsafe {
        let _ = SendMessageW(target, WM_KEYDOWN, WPARAM(VK_RETURN_WPARAM), LPARAM(0));
        let _ = SendMessageW(target, WM_KEYUP, WPARAM(VK_RETURN_WPARAM), LPARAM(0));
    }
}

fn text_matches(hwnd: HWND, expected: &str) -> bool {
    let actual = get_window_text(hwnd);
    normalize_path(&actual) == normalize_path(expected)
}

fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }

        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buf[..copied as usize])
        }
    }
}

fn normalize_path(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase()
}

fn try_keyboard_navigate(dialog: HWND, target_path: &str) -> bool {
    unsafe {
        let _ = SetForegroundWindow(dialog);
    }

    std::thread::sleep(Duration::from_millis(20));

    if !send_key_combo(VK_ALT, VK_D) {
        return false;
    }

    std::thread::sleep(Duration::from_millis(20));

    // Address bar selection is occasionally lost during rapid repeated injections.
    // Force replacement semantics so new path never gets prepended to stale text.
    if !send_key_combo(VK_CONTROL, VK_A) {
        return false;
    }

    std::thread::sleep(Duration::from_millis(10));

    if !send_vk(VK_BACK) {
        return false;
    }

    std::thread::sleep(Duration::from_millis(10));

    if !send_unicode_text(target_path) {
        return false;
    }

    send_vk(VK_RETURN_WPARAM as u16)
}

fn send_key_combo(modifier: u16, key: u16) -> bool {
    let inputs = vec![
        keyboard_input(modifier, false, false),
        keyboard_input(key, false, false),
        keyboard_input(key, true, false),
        keyboard_input(modifier, true, false),
    ];
    send_inputs(&inputs)
}

fn send_vk(vk: u16) -> bool {
    let inputs = vec![
        keyboard_input(vk, false, false),
        keyboard_input(vk, true, false),
    ];
    send_inputs(&inputs)
}

fn send_unicode_text(text: &str) -> bool {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.chars().count() * 2);
    for ch in text.encode_utf16() {
        inputs.push(keyboard_unicode_input(ch, false));
        inputs.push(keyboard_unicode_input(ch, true));
    }

    send_inputs(&inputs)
}

fn send_inputs(inputs: &[INPUT]) -> bool {
    if inputs.is_empty() {
        return false;
    }

    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

fn keyboard_input(vk: u16, key_up: bool, unicode: bool) -> INPUT {
    let mut flags = Default::default();
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    if unicode {
        flags |= KEYEVENTF_UNICODE;
    }

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn keyboard_unicode_input(unit: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
