//! 全局低层键盘钩子（`WH_KEYBOARD_LL`）。
//!
//! 悬浮窗为非激活窗口（见 [`crate::os::window_ext`]），永远拿不到 OS 键盘焦点，
//! 故搜索框无法通过 egui 的 `TextEdit` 接收输入。这里用一个全局低层键盘钩子
//! 在“悬浮条可见且目标对话框前台”时**截获**打字/导航键，转成 [`KeyAction`]
//! 送回 UI 线程驱动纯渲染的 egui；其余按键一律透传给对话框。
//!
//! 门控（`ACTIVE`）之外的按键绝不吞掉，这是避免“全局吞键”事故的护栏。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK, VK_CONTROL,
    VK_DOWN, VK_ESCAPE, VK_MENU, VK_RETURN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

/// 用户在悬浮条上产生的一次输入意图。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyAction {
    Char(char),
    Backspace,
    Up,
    Down,
    Enter,
    Escape,
}

/// 仅当为真时钩子才会截获（吞掉）消费集内的按键；否则一律透传。
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn sender_slot() -> &'static Mutex<Option<Sender<KeyAction>>> {
    static SLOT: OnceLock<Mutex<Option<Sender<KeyAction>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn ctx_slot() -> &'static Mutex<Option<egui::Context>> {
    static SLOT: OnceLock<Mutex<Option<egui::Context>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// 设置门控：悬浮条可见且目标对话框前台时为 true。
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
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
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HMODULE(hmodule.0), 0)
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
        let n = ToUnicodeEx(vk, scan, &keystate, &mut buf, 0x4, hkl);
        if n == 1 {
            let c = char::from_u32(u32::from(buf[0]))?;
            if c.is_control() { None } else { Some(c) }
        } else {
            None
        }
    }
}

/// 依据虚拟键与修饰键决定这次按下要产生的动作；返回 None 表示不消费（透传）。
fn classify(vk: u32) -> Option<KeyAction> {
    // Ctrl/Alt 组合一律透传，保留对话框自身快捷键（如 Alt+D、Ctrl+A）。
    if is_key_down(VK_CONTROL) || is_key_down(VK_MENU) {
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

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let pass = || unsafe { CallNextHookEx(HHOOK(0), code, wparam, lparam) };

    if code != HC_ACTION as i32 || !ACTIVE.load(Ordering::Relaxed) {
        return pass();
    }

    let is_key_down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
    if !is_key_down {
        return pass();
    }

    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = info.vkCode;

    let action = match classify(vk) {
        Some(a) => a,
        None => match translate_char(vk, info.scanCode) {
            Some(c) => KeyAction::Char(c),
            None => return pass(),
        },
    };

    if let Ok(guard) = sender_slot().lock()
        && let Some(sender) = guard.as_ref()
    {
        let _ = sender.send(action);
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
