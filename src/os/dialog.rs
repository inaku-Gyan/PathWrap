use log::{error, info, warn};
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
enum SendMessageAction {
    CdmSetControlText,
    EditSetTextAndEnter,
    KeyboardNavigate,
}

/// 统一记录每个策略的成功或失败结果。
fn _log_inj_result(success: bool, action: SendMessageAction) {
    if success {
        info!("Injection strategy succeeded: {:?}.", action);
    } else {
        warn!(
            "Injection strategy failed: {:?}. Trying next strategy...",
            action
        );
    }
}

/// 注入主入口：校验参数后按策略顺序尝试，直到某个策略成功。
pub fn inject_folder_path(dialog_hwnd: isize, target_path: &str) {
    if dialog_hwnd == 0 {
        error!("invalid dialog hwnd: 0");
        return;
    }
    if target_path.trim().is_empty() {
        error!("target path is empty");
        return;
    }

    let dialog = HWND(dialog_hwnd);

    info!(
        "Injection starts: hwnd={} target='{}'",
        dialog_hwnd, target_path
    );

    unsafe {
        SetForegroundWindow(dialog);
    }

    if try_cdm_setcontroltext(dialog, target_path) {
        _log_inj_result(true, SendMessageAction::CdmSetControlText);
        return;
    }
    _log_inj_result(false, SendMessageAction::CdmSetControlText);

    if try_edit_settext_enter(dialog, target_path) {
        _log_inj_result(true, SendMessageAction::EditSetTextAndEnter);
        return;
    }
    _log_inj_result(false, SendMessageAction::EditSetTextAndEnter);

    if try_keyboard_navigate(dialog, target_path) {
        _log_inj_result(true, SendMessageAction::KeyboardNavigate);
        return;
    }
    _log_inj_result(false, SendMessageAction::KeyboardNavigate);

    error!(
        "Injection failed: all strategies exhausted. hwnd={} target='{}'",
        dialog_hwnd, target_path
    );
}

/// 策略 1: 通过 CDM_SETCONTROLTEXT 写入旧式对话框控件文本并尝试确认。
fn try_cdm_setcontroltext(dialog: HWND, target_path: &str) -> bool {
    let text = to_wide_null(target_path);

    unsafe {
        SendMessageW(
            dialog,
            CDM_SETCONTROLTEXT,
            WPARAM(EDT1),
            LPARAM(text.as_ptr() as isize),
        );
    }

    if let Some(edit) = find_filename_edit(dialog) {
        if text_matches(edit, target_path) {
            trigger_confirm(dialog, edit)
        } else {
            let actual = get_window_text(edit);
            warn!(
                "CdmSetControlText validation mismatch: expected='{}' actual='{}'",
                target_path, actual
            );
            false
        }
    } else {
        warn!("CdmSetControlText failed: filename edit control not found");
        false
    }
}

/// 策略 2: 直接向 Edit 控件发送 WM_SETTEXT，然后触发确认。
fn try_edit_settext_enter(dialog: HWND, target_path: &str) -> bool {
    let Some(edit) = find_filename_edit(dialog) else {
        warn!("EditSetTextAndEnter failed: filename edit control not found");
        return false;
    };
    let text = to_wide_null(target_path);

    unsafe {
        let _ = SendMessageW(edit, WM_SETTEXT, WPARAM(0), LPARAM(text.as_ptr() as isize));
    }

    if text_matches(edit, target_path) {
        trigger_confirm(dialog, edit)
    } else {
        let actual = get_window_text(edit);
        warn!(
            "EditSetTextAndEnter validation mismatch: expected='{}' actual='{}'",
            target_path, actual
        );
        false
    }
}

/// 在文件对话框中查找可能的文件名/地址输入 Edit 控件。
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

/// 触发提交动作：向输入控件与对话框发送回车，并补发 IDOK。
fn trigger_confirm(dialog: HWND, edit: HWND) -> bool {
    send_enter(edit);
    send_enter(dialog);

    unsafe {
        let _ = SendMessageW(dialog, WM_COMMAND, WPARAM(IDOK_WPARAM), LPARAM(0));
    }

    true
}

/// 向指定窗口句柄发送一次回车按键按下/抬起消息。
fn send_enter(target: HWND) {
    unsafe {
        let _ = SendMessageW(target, WM_KEYDOWN, WPARAM(VK_RETURN_WPARAM), LPARAM(0));
        let _ = SendMessageW(target, WM_KEYUP, WPARAM(VK_RETURN_WPARAM), LPARAM(0));
    }
}

/// 比较控件中的实际文本与目标路径是否一致（归一化后）。
fn text_matches(hwnd: HWND, expected: &str) -> bool {
    let actual = get_window_text(hwnd);
    normalize_path(&actual) == normalize_path(expected)
}

/// 读取窗口控件文本，供策略校验与诊断日志使用。
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

/// 归一化路径字符串，降低大小写、引号与分隔符差异带来的误判。
fn normalize_path(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase()
}

/// 策略 3: 模拟真实键盘导航（Alt+D、Ctrl+A、Backspace、输入路径、Enter）。
fn try_keyboard_navigate(dialog: HWND, target_path: &str) -> bool {
    unsafe {
        let _ = SetForegroundWindow(dialog);
    }

    std::thread::sleep(Duration::from_millis(20));

    if !send_key_combo(VK_ALT, VK_D) {
        warn!("KeyboardNavigate failed: cannot focus address bar with Alt+D");
        return false;
    }

    std::thread::sleep(Duration::from_millis(20));

    // Address bar selection is occasionally lost during rapid repeated injections.
    // Force replacement semantics so new path never gets prepended to stale text.
    if !send_key_combo(VK_CONTROL, VK_A) {
        warn!("KeyboardNavigate failed: cannot select all with Ctrl+A");
        return false;
    }

    std::thread::sleep(Duration::from_millis(10));

    if !send_vk(VK_BACK) {
        warn!("KeyboardNavigate failed: cannot clear existing text with Backspace");
        return false;
    }

    std::thread::sleep(Duration::from_millis(10));

    if !send_unicode_text(target_path) {
        warn!("KeyboardNavigate failed: cannot input target path text");
        return false;
    }

    if !send_vk(VK_RETURN_WPARAM as u16) {
        warn!("KeyboardNavigate failed: cannot submit path with Enter");
        return false;
    }

    true
}

/// 发送组合键（如 Alt+D、Ctrl+A）。
fn send_key_combo(modifier: u16, key: u16) -> bool {
    let inputs = vec![
        keyboard_input(modifier, false, false),
        keyboard_input(key, false, false),
        keyboard_input(key, true, false),
        keyboard_input(modifier, true, false),
    ];
    send_inputs(&inputs)
}

/// 发送单个虚拟键（按下+抬起）。
fn send_vk(vk: u16) -> bool {
    let inputs = vec![
        keyboard_input(vk, false, false),
        keyboard_input(vk, true, false),
    ];
    send_inputs(&inputs)
}

/// 以 Unicode 按键事件逐字符输入文本。
fn send_unicode_text(text: &str) -> bool {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.chars().count() * 2);
    for ch in text.encode_utf16() {
        inputs.push(keyboard_unicode_input(ch, false));
        inputs.push(keyboard_unicode_input(ch, true));
    }

    send_inputs(&inputs)
}

/// 调用 SendInput 批量发送输入事件，并检查是否全部发送成功。
fn send_inputs(inputs: &[INPUT]) -> bool {
    if inputs.is_empty() {
        return false;
    }

    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

/// 构建虚拟键输入事件，可配置 keyup 与 unicode 标志。
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

/// 构建 Unicode 字符输入事件，用于稳定输入路径文本。
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

/// 将 Rust 字符串转换为 Win32 需要的 UTF-16 结尾零缓冲区。
fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
