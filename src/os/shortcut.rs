//! A native Ctrl+G notification belongs to one live dialog interaction. Register
//! only while its dialog/overlay pair is foreground; retire the registration
//! when that changes. Handle normal activation in the native hotkey notification
//! and report the actual result; a late UI result cannot initiate activation.
use crate::core::controller::{FocusRequest, FocusResult};
use std::cell::RefCell;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    IsWindowEnabled, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_G,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EVENT_SYSTEM_FOREGROUND, GetMessageW,
    GetWindowThreadProcessId, IsWindowVisible, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW,
    PostQuitMessage, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_HOTKEY, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};

const SCOPE_CHANGED: u32 = WM_APP + 1;
const LAST_HOTKEY_ID: i32 = 0xbfff;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Scope {
    request: FocusRequest,
    overlay: isize,
}

impl Scope {
    fn is_foreground(self) -> bool {
        let dialog = HWND(self.request.dialog.hwnd as *mut core::ffi::c_void);
        let overlay = HWND(self.overlay as *mut core::ffi::c_void);
        unsafe {
            if !IsWindowVisible(dialog).as_bool()
                || !IsWindowEnabled(dialog).as_bool()
                || !IsWindowVisible(overlay).as_bool()
            {
                return false;
            }
            let mut overlay_pid = 0;
            GetWindowThreadProcessId(overlay, Some(&mut overlay_pid));
            if overlay_pid != GetCurrentProcessId()
                || super::dialog::identity(self.request.dialog.hwnd) != Some(self.request.dialog)
            {
                return false;
            }
            let foreground = super::window_ext::foreground_hwnd();
            foreground == self.request.dialog.hwnd || foreground == self.overlay
        }
    }
}

#[derive(Default)]
struct Shared {
    scope: Option<Scope>,
    pending: Option<FocusResult>,
    availability: Option<bool>,
}

pub struct Listener {
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
}

impl Listener {
    pub fn start() -> Result<Self, String> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let worker_shared = Arc::clone(&shared);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("pathwarp-shortcut".into())
            .spawn(move || run(worker_shared, ready_tx))
            .map_err(|error| error.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                shared,
                thread: Some(thread),
                thread_id,
            }),
            result => {
                let _ = thread.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "shortcut thread ended before startup".into(),
                })
            }
        }
    }

    pub fn set_scope(&self, request: Option<FocusRequest>, overlay: isize) {
        let scope = request
            .filter(|_| overlay != 0)
            .map(|request| Scope { request, overlay });
        let changed = if let Ok(mut shared) = self.shared.lock() {
            if shared.scope == scope {
                false
            } else {
                shared.scope = scope;
                true
            }
        } else {
            false
        };
        if changed {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, SCOPE_CHANGED, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn take_result(&self) -> Option<FocusResult> {
        self.shared.lock().ok()?.pending.take()
    }

    pub fn take_availability(&self) -> Option<bool> {
        self.shared.lock().ok()?.availability.take()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.set_scope(None, 0);
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone, Copy)]
struct Binding {
    id: i32,
    scope: Scope,
}

impl Binding {
    fn request(self, id: usize, current: Option<Scope>) -> Option<FocusRequest> {
        (usize::try_from(self.id).ok() == Some(id) && current == Some(self.scope))
            .then_some(self.scope.request)
    }
}

struct Router {
    shared: Arc<Mutex<Shared>>,
    binding: Option<Binding>,
    failed: Option<Scope>,
    next_id: i32,
    retiring: bool,
}

impl Router {
    fn current_scope(&self) -> Option<Scope> {
        self.shared
            .lock()
            .ok()?
            .scope
            .filter(|scope| scope.is_foreground())
    }

    fn available(&self, available: bool) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.availability = Some(available);
        }
    }

    fn unregister(&mut self) -> bool {
        if let Some(binding) = self.binding.take()
            && let Err(error) = unsafe { UnregisterHotKey(None, binding.id) }
        {
            log::warn!("could not retire Ctrl+G registration: {error}");
            self.available(false);
            self.retiring = true;
            // Retire the owning thread rather than keep a global registration
            // alive after its foreground/session scope has ended.
            unsafe {
                PostQuitMessage(0);
            }
            return false;
        }
        true
    }

    fn synchronize(&mut self) {
        if self.retiring {
            return;
        }
        let scope = self.current_scope();
        if self.binding.map(|binding| binding.scope) == scope && self.failed.is_none() {
            return;
        }
        if self.failed == scope && self.failed.is_some() {
            return;
        }
        if !self.unregister() {
            return;
        }
        self.failed = None;
        let Some(scope) = scope else {
            self.available(true);
            return;
        };
        // Never reuse a retired id: a queued WM_HOTKEY cannot be rebound to a
        // different session, even if its HWND is reused. Exhaustion fails closed.
        if self.next_id > LAST_HOTKEY_ID {
            self.failed = Some(scope);
            self.available(false);
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        match unsafe { RegisterHotKey(None, id, MOD_CONTROL | MOD_NOREPEAT, u32::from(VK_G.0)) } {
            Ok(()) => {
                self.binding = Some(Binding { id, scope });
                self.available(true);
            }
            Err(error) => {
                log::warn!("Ctrl+G registration is unavailable: {error}");
                self.failed = Some(scope);
                self.available(false);
            }
        }
    }

    fn hotkey(&mut self, id: usize) {
        self.synchronize();
        let current = self.current_scope();
        if let Some(request) = self
            .binding
            .and_then(|binding| binding.request(id, current))
            && let Some(scope) = current
        {
            // This is the sole activation attempt for the chord, at its native
            // input notification rather than after a UI-frame/channel delay.
            let activated = super::window_ext::activate(scope.overlay);
            if let Ok(mut shared) = self.shared.lock()
                && shared.scope == Some(scope)
            {
                shared.pending = Some(FocusResult { request, activated });
            }
        }
    }
}

thread_local! {
    static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    ROUTER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(router) = slot.as_mut()
        {
            router.synchronize();
        } else {
            // A foreground notification can reenter native activation. Keep
            // that notification rather than lose the unregister transition.
            unsafe {
                let _ =
                    PostThreadMessageW(GetCurrentThreadId(), SCOPE_CHANGED, WPARAM(0), LPARAM(0));
            }
        }
    });
}

unsafe extern "system" fn keyboard_scope_guard(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == 0
        && lparam.0 != 0
        && matches!(u32::try_from(wparam.0), Ok(WM_KEYDOWN | WM_SYSKEYDOWN))
        && unsafe { (*(lparam.0 as *const KBDLLHOOKSTRUCT)).vkCode } == u32::from(VK_G.0)
    {
        ROUTER.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut()
                && let Some(router) = slot.as_mut()
                && let Some(binding) = router.binding
            {
                // Foreground WinEvents are asynchronous. Retire an obsolete
                // registration BEFORE Windows matches this key to a hotkey,
                // so the first Ctrl+G in another app reaches that app too.
                // Never wait for the UI, consume a key or activate here.
                let current = router
                    .shared
                    .try_lock()
                    .ok()
                    .and_then(|shared| shared.scope);
                if current != Some(binding.scope) || !binding.scope.is_foreground() {
                    router.unregister();
                    unsafe {
                        let _ = PostThreadMessageW(
                            GetCurrentThreadId(),
                            SCOPE_CHANGED,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
            }
        });
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn run(shared: Arc<Mutex<Shared>>, ready: mpsc::SyncSender<Result<u32, String>>) {
    ROUTER.with(|slot| {
        *slot.borrow_mut() = Some(Router {
            shared,
            binding: None,
            failed: None,
            next_id: 1,
            retiring: false,
        })
    });
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.is_invalid() {
        let _ = ready.send(Err("could not observe foreground changes for Ctrl+G".into()));
    } else {
        let guard = unsafe {
            GetModuleHandleW(None).and_then(|module| {
                SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_scope_guard),
                    Some(module.into()),
                    0,
                )
            })
        };
        match guard {
            Ok(guard) => {
                let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
                while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                    ROUTER.with(|slot| {
                        if let Some(router) = slot.borrow_mut().as_mut() {
                            match message.message {
                                SCOPE_CHANGED => router.synchronize(),
                                WM_HOTKEY => router.hotkey(message.wParam.0),
                                _ => {}
                            }
                        }
                    });
                    unsafe {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                unsafe {
                    let _ = UnhookWindowsHookEx(guard);
                }
            }
            Err(error) => {
                let _ = ready.send(Err(format!("could not guard Ctrl+G scope: {error}")));
            }
        }
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
    ROUTER.with(|slot| {
        if let Some(mut router) = slot.borrow_mut().take() {
            router.unregister();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::navigation::DialogIdentity;

    fn scope(session: u64) -> Scope {
        Scope {
            request: FocusRequest {
                session,
                dialog: DialogIdentity {
                    hwnd: 1,
                    pid: 2,
                    process_created: 3,
                },
            },
            overlay: 4,
        }
    }

    #[test]
    fn queued_hotkeys_require_the_original_registration_and_session() {
        let binding = Binding {
            id: 1,
            scope: scope(1),
        };
        assert_eq!(binding.request(1, Some(scope(1))), Some(scope(1).request));
        assert_eq!(binding.request(2, Some(scope(1))), None);
        assert_eq!(binding.request(1, Some(scope(2))), None);
        assert_eq!(binding.request(1, None), None);
    }

    #[test]
    fn replacement_window_identity_does_not_match_a_queued_hotkey() {
        let binding = Binding {
            id: 1,
            scope: scope(1),
        };
        let mut replacement = scope(1);
        replacement.request.dialog.process_created += 1;
        assert_eq!(binding.request(1, Some(replacement)), None);
    }

    #[test]
    fn idle_listener_starts_and_joins_without_registering_a_hotkey() {
        let Ok(listener) = Listener::start() else {
            panic!("foreground observer unavailable");
        };
        assert!(listener.take_result().is_none());
        assert!(listener.take_availability().is_none());
        drop(listener);
    }
}
