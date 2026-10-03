//! 全局低层键盘钩子（`WH_KEYBOARD_LL`）。
//!
//! 悬浮窗为非激活窗口（见 [`crate::os::window_ext`]），永远拿不到 OS 键盘焦点，
//! 故搜索框无法通过 egui 的 `TextEdit` 接收输入。这里用一个全局低层键盘钩子
//! 在用户点击悬浮层搜索/列表后进入显式捕获态，把打字/导航键转成
//! [`KeyAction`] 送回 UI 线程驱动纯渲染的 egui；未显式捕获、鼠标回到对话框、
//! 输入法/系统快捷键和钩子通道不可用时，所有按键一律透传给原目标窗口。
//!
//! 门控（`ACTIVE`）之外的按键绝不吞掉，这是避免“全局吞键”事故的护栏。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK, VK_CONTROL,
    VK_DOWN, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowRect, HC_ACTION,
    KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_SYSKEYDOWN,
};

pub use crate::core::types::KeyAction;

/// 对话框会话可用时的钩子门控；它本身不代表悬浮层已经获得输入捕获。
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// 只有用户点击悬浮层搜索/列表后才进入捕获态；对话框前台本身不等于捕获态。
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// 用于判断捕获态下鼠标是否仍停在悬浮层内；移到对话框后自动 fail-open。
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct KeyModifiers {
    ctrl: bool,
    alt: bool,
    win: bool,
}

fn sender_slot() -> &'static Mutex<Option<Sender<KeyAction>>> {
    static SLOT: OnceLock<Mutex<Option<Sender<KeyAction>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn ctx_slot() -> &'static Mutex<Option<egui::Context>> {
    static SLOT: OnceLock<Mutex<Option<egui::Context>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// 设置会话门控：悬浮条可见且目标对话框前台时为 true。
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
    if !active {
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
    }
}

/// 更新悬浮层 HWND，供低层钩子判断鼠标是否仍在悬浮层区域。
pub fn set_overlay_hwnd(hwnd: isize) {
    OVERLAY_HWND.store(hwnd, Ordering::Relaxed);
}

/// 由悬浮层鼠标交互显式开启搜索输入捕获；点击回对话框后由键盘钩子自动关闭。
pub fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::Relaxed);
}

/// Read whether the non-activating overlay is currently armed for keyboard input.
pub fn capture_active() -> bool {
    CAPTURE_ACTIVE.load(Ordering::Relaxed)
}

/// 安装全局键盘钩子并返回接收 [`KeyAction`] 的通道。钩子运行于独立线程。
pub fn install(ctx: egui::Context) -> Receiver<KeyAction> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        if let Ok(mut guard) = sender_slot().lock() {
            *guard = Some(tx);
        }
        if let Ok(mut guard) = ctx_slot().lock() {
            *guard = Some(ctx);
        }

        let hmodule = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
        let hook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                Some(HINSTANCE(hmodule.0)),
                0,
            )
        };

        let hook = match hook {
            Ok(h) => h,
            Err(err) => {
                log::error!("failed to install WH_KEYBOARD_LL hook: {err}");
                return;
            }
        };

        // 低层钩子要求安装线程有消息循环。
        let mut msg = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if result.0 <= 0 {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    });

    rx
}

fn is_key_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(i32::from(vk.0)) < 0 }
}

fn current_modifiers() -> KeyModifiers {
    KeyModifiers {
        ctrl: is_key_down(VK_CONTROL),
        alt: is_key_down(VK_MENU),
        win: is_key_down(VK_LWIN) || is_key_down(VK_RWIN),
    }
}

fn cursor_is_over_overlay() -> bool {
    let hwnd = OVERLAY_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return false;
    }

    unsafe {
        let mut point = POINT::default();
        let mut rect = RECT::default();
        if GetCursorPos(&mut point).is_err()
            || GetWindowRect(hwnd_from_raw(hwnd), &mut rect).is_err()
        {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}

fn hwnd_from_raw(hwnd: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void)
}

/// 把虚拟键翻译成一个可打印字符（考虑当前键盘布局），控制字符返回 None。
fn translate_char(vk: u32, scan: u32) -> Option<char> {
    unsafe {
        let mut keystate = [0u8; 256];
        if GetKeyboardState(&mut keystate).is_err() {
            return None;
        }
        let hkl = GetKeyboardLayout(0);
        let mut buf = [0u16; 8];
        // wFlags bit 2 (0x4): 不改变键盘状态（Win10 1607+），避免影响死键组合。
        let n = ToUnicodeEx(vk, scan, &keystate, &mut buf, 0x4, Some(hkl));
        if n == 1 {
            let c = char::from_u32(u32::from(buf[0]))?;
            if c.is_control() { None } else { Some(c) }
        } else {
            None
        }
    }
}

/// 依据虚拟键与修饰键决定这次按下要产生的动作；返回 None 表示不消费（透传）。
fn classify(vk: u32, modifiers: KeyModifiers) -> Option<KeyAction> {
    // Ctrl/Alt/Win 组合一律透传，保留对话框自身快捷键和系统全局快捷键。
    if modifiers.ctrl || modifiers.alt || modifiers.win {
        return None;
    }

    match vk {
        v if v == u32::from(VK_ESCAPE.0) => Some(KeyAction::Escape),
        v if v == u32::from(VK_RETURN.0) => Some(KeyAction::Enter),
        v if v == u32::from(VK_BACK.0) => Some(KeyAction::Backspace),
        v if v == u32::from(VK_UP.0) => Some(KeyAction::Up),
        v if v == u32::from(VK_DOWN.0) => Some(KeyAction::Down),
        _ => None,
    }
}

/// 将一个低层键盘事件路由到悬浮层输入状态机。
///
/// 这层保持纯函数，避免把“钩子已启用”误当成“本次按键应被消费”。调用方
/// 仍需在真正交给 UI 后才返回非零值阻止目标窗口收到该事件。
fn route_key(
    active: bool,
    capture: bool,
    modifiers: KeyModifiers,
    vk: u32,
    translated: Option<char>,
) -> Option<KeyAction> {
    if !active || !capture || modifiers.ctrl || modifiers.alt || modifiers.win {
        return None;
    }

    match classify(vk, modifiers) {
        Some(action) => Some(action),
        None => translated.map(KeyAction::Char),
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let pass = || unsafe { CallNextHookEx(None, code, wparam, lparam) };

    let active = ACTIVE.load(Ordering::Relaxed);
    let capture = CAPTURE_ACTIVE.load(Ordering::Relaxed);
    if code != HC_ACTION as i32 || !active || !capture {
        return pass();
    }

    let is_key_down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
    if !is_key_down {
        return pass();
    }

    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = info.vkCode;
    let modifiers = current_modifiers();

    // The Win key is a system shortcut boundary. Release search capture as soon as
    // it appears, so Win+Space/Win+R and similar shortcuts are never swallowed.
    if vk == u32::from(VK_LWIN.0) || vk == u32::from(VK_RWIN.0) {
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        return pass();
    }

    // Ctrl/Alt are also shortcut and IME boundaries. Release capture before
    // forwarding them so the following key in a toggle chord cannot be routed
    // into the overlay even while the pointer remains over it.
    if modifiers.ctrl || modifiers.alt {
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        return pass();
    }

    // A non-activating overlay has no real keyboard focus. Treat moving the
    // pointer back to the dialog as the user's explicit request to return input
    // to that dialog, and fail open before consuming its first key.
    if !cursor_is_over_overlay() {
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        return pass();
    }

    let translated = if classify(vk, modifiers).is_none() {
        translate_char(vk, info.scanCode)
    } else {
        None
    };
    let action = match route_key(active, capture, modifiers, vk, translated) {
        Some(action) => action,
        None => return pass(),
    };

    let delivered = sender_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|sender| sender.send(action).is_ok()))
        .unwrap_or(false);
    if !delivered {
        return pass();
    }
    if let Ok(guard) = ctx_slot().lock()
        && let Some(ctx) = guard.as_ref()
    {
        ctx.request_repaint();
    }

    // 吞掉本次按下：keydown 不进入对话框消息队列，也就不会生成 WM_CHAR，
    // 从而文本不会泄漏到对话框（keyup 无害，任其透传）。
    LRESULT(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VK_A: u32 = 0x41;

    #[test]
    fn active_hook_must_not_consume_plain_text_without_capture() {
        assert_eq!(
            route_key(true, false, KeyModifiers::default(), VK_A, Some('a')),
            None,
            "plain text must pass through until search capture is explicitly armed"
        );
    }

    #[test]
    fn win_shortcuts_must_pass_through() {
        assert_eq!(
            route_key(
                true,
                true,
                KeyModifiers {
                    win: true,
                    ..KeyModifiers::default()
                },
                VK_A,
                Some('a'),
            ),
            None,
            "Win shortcuts must never be converted into overlay text"
        );
    }

    #[test]
    fn ctrl_and_alt_shortcuts_must_pass_through() {
        for modifiers in [
            KeyModifiers {
                ctrl: true,
                ..KeyModifiers::default()
            },
            KeyModifiers {
                alt: true,
                ..KeyModifiers::default()
            },
        ] {
            assert_eq!(
                route_key(true, true, modifiers, VK_A, Some('a')),
                None,
                "Ctrl/Alt shortcuts must never be converted into overlay text"
            );
        }
    }

    #[test]
    fn armed_plain_text_is_routed_to_overlay() {
        assert_eq!(
            route_key(true, true, KeyModifiers::default(), VK_A, Some('a')),
            Some(KeyAction::Char('a'))
        );
    }
}
