//! Owned native settings/tray windows on one message thread. Callbacks enqueue
//! application commands and wake hidden ROOT; they never reenter the egui app.
use crate::config::ThemePreference;
use crate::settings::View;
use crate::startup::Action;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateFontW, DEFAULT_CHARSET,
    DEFAULT_PITCH, DeleteObject, GetMonitorInfoW, GetSysColorBrush, HFONT,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, OUT_DEFAULT_PRECIS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::Shell::NIF_SHOWTIP;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT,
    NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::WM_CANCELMODE;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BM_SETCHECK, BS_AUTORADIOBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreatePopupMenu,
    CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_READONLY, GWLP_USERDATA, GetClassInfoW, GetClientRect, GetCursorPos,
    GetDlgItem, GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW, IDI_APPLICATION,
    IsDialogMessageW, IsIconic, IsWindow, LoadCursorW, LoadIconW, MF_STRING, MINMAXINFO, MSG,
    PostMessageW, PostThreadMessageW, RegisterClassW, RegisterWindowMessageW, SW_SHOWNA,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenuEx, TranslateMessage, WA_INACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE,
    WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DPICHANGED, WM_GETMINMAXINFO, WM_NCCREATE,
    WM_NCDESTROY, WM_NULL, WM_QUIT, WM_SETFONT, WM_SIZE, WNDCLASSW, WS_CHILD, WS_EX_APPWINDOW,
    WS_EX_CONTROLPARENT, WS_GROUP, WS_OVERLAPPED, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const WAKE: u32 = WM_APP + 20;
const OPEN: u32 = WM_APP + 21;
const SYNC: u32 = WM_APP + 22;
const TRAY: u32 = WM_APP + 23;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
pub const SETTINGS_CLASS: &str = "PathWarp.Settings";
pub const TRAY_CLASS: &str = "PathWarp.Tray";
const AUTO: i32 = 101;
const LIGHT: i32 = 102;
const DARK: i32 = 103;
const THEME_STATUS: i32 = 104;
const RETRY: i32 = 105;
const STARTUP_STATUS: i32 = 106;
const USE_CURRENT: i32 = 107;
const REMOVE: i32 = 108;
const REFRESH: i32 = 109;
const FOCUS_STATUS: i32 = 110;

#[derive(Debug)]
pub enum Command {
    Interaction(bool),
    Opened,
    Theme(ThemePreference),
    RetrySave,
    Startup(Action, u64),
    Refresh,
    Diagnostic(Option<String>),
}

enum NativeEvent {
    Menu(POINT),
    Layout,
    RecreateTray,
}

struct Shared {
    ctx: egui::Context,
    commands: mpsc::Sender<Command>,
    native: mpsc::Sender<NativeEvent>,
    thread_id: u32,
    overlay: AtomicIsize,
    settings: AtomicIsize,
    owner: AtomicIsize,
    menu_active: AtomicBool,
    window_active: AtomicBool,
    open_pending: AtomicBool,
    view: Mutex<View>,
    /// Only a completed native presentation can authorize its button actions.
    rendered_revision: Mutex<Option<u64>>,
    cancellation: Mutex<Option<Arc<AtomicBool>>>,
    shell_message: u32,
}

impl Shared {
    fn startup_command(&self, action: Action) -> Option<Command> {
        let revision = (*self.rendered_revision.lock().ok()?)?;
        Some(Command::Startup(action, revision))
    }

    fn present(&self, render: impl FnOnce(&View)) {
        let Ok(view) = self.view.lock().map(|view| view.clone()) else {
            return;
        };
        let Ok(mut revision) = self.rendered_revision.lock() else {
            return;
        };
        *revision = None;
        drop(revision);
        render(&view);
        if let Ok(mut revision) = self.rendered_revision.lock() {
            // ROOT may publish a newer view during rendering. Bind the click
            // to these controls, so the registration model rejects it as stale.
            *revision = Some(view.revision);
        }
    }

    fn begin_settings_open(&self) {
        self.open_pending.store(true, Ordering::Release);
        self.interact();
        self.send(Command::Opened);
    }

    fn finish_settings_open(&self, focused: bool) {
        self.window_active.store(focused, Ordering::Release);
        if focused {
            self.open_pending.store(false, Ordering::Release);
        }
        self.interact();
    }
    fn settings_focus_changed(&self, active: bool) {
        if active {
            self.open_pending.store(false, Ordering::Release);
        }
        self.window_active.store(active, Ordering::Release);
        self.interact();
    }
    fn settings_closed(&self) {
        if let Ok(mut revision) = self.rendered_revision.lock() {
            *revision = None;
        }
        self.open_pending.store(false, Ordering::Release);
        self.window_active.store(false, Ordering::Release);
        self.interact();
    }
    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
        self.ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    fn native(&self, event: NativeEvent) {
        let _ = self.native.send(event);
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WAKE, WPARAM(0), LPARAM(0));
        }
    }
    fn interact(&self) {
        let active = self.menu_active.load(Ordering::Acquire)
            || self.window_active.load(Ordering::Acquire)
            || self.open_pending.load(Ordering::Acquire);
        if active {
            if let Ok(cancel) = self.cancellation.lock()
                && let Some(cancel) = cancel.as_ref()
            {
                cancel.store(true, Ordering::Release);
            }
            let overlay = self.overlay.load(Ordering::Acquire);
            if overlay != 0 {
                super::window_ext::park(overlay);
            }
        }
        self.send(Command::Interaction(active));
    }
}

pub struct Desktop {
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Desktop {
    pub fn start(ctx: egui::Context, overlay: isize, view: View) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("pathwarp-settings".into())
            .spawn(move || {
                let (native, native_rx) = mpsc::channel();
                let shared = Arc::new(Shared {
                    ctx,
                    commands,
                    native,
                    thread_id: unsafe { GetCurrentThreadId() },
                    overlay: AtomicIsize::new(overlay),
                    settings: AtomicIsize::new(0),
                    owner: AtomicIsize::new(0),
                    menu_active: AtomicBool::new(false),
                    window_active: AtomicBool::new(false),
                    open_pending: AtomicBool::new(false),
                    view: Mutex::new(view),
                    rendered_revision: Mutex::new(None),
                    cancellation: Mutex::new(None),
                    shell_message: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
                });
                let result =
                    create_window(&shared, w!("PathWarp.Tray"), w!("PathWarp tray"), false);
                match result {
                    Ok(owner) => {
                        shared.owner.store(owner.0 as isize, Ordering::Release);
                        let _ = ready_tx.send(Ok(Arc::clone(&shared)));
                        run(shared, owner, native_rx);
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(shared)) => Ok(Self {
                shared,
                receiver,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "设置消息线程启动失败。".into(),
                })
            }
        }
    }

    pub fn drain(&self) -> Vec<Command> {
        self.receiver.try_iter().collect()
    }
    pub fn open(&self) {
        unsafe {
            let _ = PostThreadMessageW(self.shared.thread_id, OPEN, WPARAM(0), LPARAM(0));
        }
    }
    pub fn set_cancellation(&self, cancel: Option<Arc<AtomicBool>>) {
        if (self.shared.window_active.load(Ordering::Acquire)
            || self.shared.menu_active.load(Ordering::Acquire)
            || self.shared.open_pending.load(Ordering::Acquire))
            && let Some(cancel) = &cancel
        {
            cancel.store(true, Ordering::Release);
        }
        if let Ok(mut slot) = self.shared.cancellation.lock() {
            *slot = cancel;
        }
    }
    pub fn update(&self, view: View) {
        if let Ok(mut slot) = self.shared.view.lock()
            && *slot != view
        {
            *slot = view;
            unsafe {
                let _ = PostThreadMessageW(self.shared.thread_id, SYNC, WPARAM(0), LPARAM(0));
            }
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        unsafe {
            let owner = self.shared.owner.load(Ordering::Acquire);
            if owner != 0 {
                let _ = PostMessageW(Some(hwnd(owner)), WM_CANCELMODE, WPARAM(0), LPARAM(0));
            }
            let _ = PostThreadMessageW(self.shared.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn create_window(
    shared: &Arc<Shared>,
    class: PCWSTR,
    title: PCWSTR,
    settings: bool,
) -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class_definition = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?,
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            ..Default::default()
        };
        if GetClassInfoW(Some(module.into()), class, &mut WNDCLASSW::default()).is_err()
            && RegisterClassW(&class_definition) == 0
        {
            return Err(windows::core::Error::from_thread().to_string());
        }
        CreateWindowExW(
            if settings {
                WS_EX_APPWINDOW | WS_EX_CONTROLPARENT
            } else {
                WINDOW_EX_STYLE::default()
            },
            class,
            title,
            if settings {
                WS_OVERLAPPEDWINDOW
            } else {
                WS_OVERLAPPED
            },
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            if settings { 800 } else { 0 },
            if settings { 740 } else { 0 },
            None,
            None,
            Some(module.into()),
            Some(Arc::as_ptr(shared).cast()),
        )
        .map_err(|e| e.to_string())
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lp.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Shared;
    if !pointer.is_null() {
        let shared = unsafe { &*pointer };
        let settings = shared.settings.load(Ordering::Acquire) == window.0 as isize;
        if !settings && message == shared.shell_message && shared.shell_message != 0 {
            shared.native(NativeEvent::RecreateTray);
            return LRESULT(0);
        }
        match message {
            TRAY => {
                let event = u32::try_from(lp.0 & 0xffff).unwrap_or_default();
                if event == WM_CONTEXTMENU || event == NIN_KEYSELECT || event == NIN_SELECT {
                    let x = u16::try_from(wp.0 & 0xffff).unwrap_or_default();
                    let y = u16::try_from((wp.0 >> 16) & 0xffff).unwrap_or_default();
                    shared.native(NativeEvent::Menu(POINT {
                        x: i32::from(i16::from_le_bytes(x.to_le_bytes())),
                        y: i32::from(i16::from_le_bytes(y.to_le_bytes())),
                    }));
                }
                return LRESULT(0);
            }
            WM_ACTIVATE if settings => {
                let active = wp.0 & 0xffff != usize::try_from(WA_INACTIVE).unwrap_or_default();
                shared.settings_focus_changed(active);
                if active {
                    set_text(
                        window,
                        FOCUS_STATUS,
                        "设置已取得焦点。关闭后应用继续留在托盘。",
                    );
                }
            }
            WM_COMMAND if settings => {
                let id = i32::try_from(wp.0 & 0xffff).unwrap_or_default();
                let command = match id {
                    AUTO => Some(Command::Theme(ThemePreference::Auto)),
                    LIGHT => Some(Command::Theme(ThemePreference::Light)),
                    DARK => Some(Command::Theme(ThemePreference::Dark)),
                    RETRY => Some(Command::RetrySave),
                    USE_CURRENT => shared.startup_command(Action::UseCurrent),
                    REMOVE => shared.startup_command(Action::Remove),
                    REFRESH => Some(Command::Refresh),
                    2 => {
                        unsafe {
                            let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    shared.send(command);
                }
                return LRESULT(0);
            }
            WM_SIZE if settings => shared.native(NativeEvent::Layout),
            WM_GETMINMAXINFO if settings => {
                let dpi = unsafe { GetDpiForWindow(window) }.max(96);
                let bounds = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
                bounds.ptMinTrackSize.x = i32::try_from(600 * dpi / 96).unwrap_or(600);
                bounds.ptMinTrackSize.y = i32::try_from(500 * dpi / 96).unwrap_or(500);
            }
            WM_NCDESTROY if settings => {
                shared.settings.store(0, Ordering::Release);
                shared.settings_closed();
            }
            WM_DPICHANGED if settings => {
                let rect = unsafe { &*(lp.0 as *const RECT) };
                unsafe {
                    let _ = SetWindowPos(
                        window,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                }
                shared.native(NativeEvent::Layout);
                return LRESULT(0);
            }
            WM_CLOSE if settings => {
                shared.settings.store(0, Ordering::Release);
                shared.settings_closed();
                unsafe {
                    let _ = DestroyWindow(window);
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(window, message, wp, lp) }
}

fn notify(owner: HWND, operation: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) -> bool {
    let mut data = NOTIFYICONDATAW {
        cbSize: u32::try_from(size_of::<NOTIFYICONDATAW>()).unwrap_or_default(),
        hWnd: owner,
        uID: 1,
        // VERSION_4 suppresses standard tooltips unless SHOWTIP is explicit.
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: TRAY,
        hIcon: unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default(),
        ..Default::default()
    };
    let tip = wide("PathWarp — 右键打开设置");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    unsafe { Shell_NotifyIconW(operation, &data).as_bool() }
}

fn restore_tray(shared: &Shared, owner: HWND) {
    if notify(owner, NIM_ADD) && notify(owner, NIM_SETVERSION) {
        shared.send(Command::Diagnostic(None));
    } else {
        let _ = notify(owner, NIM_DELETE);
        shared.send(Command::Diagnostic(Some(
            "托盘入口不可用，可从悬浮层主题菜单打开设置。".into(),
        )));
    }
}

fn control(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    id: i32,
    style: WINDOW_STYLE,
) -> Result<(), String> {
    let text = wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as *mut core::ffi::c_void)),
            None,
            None,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
}

fn create_controls(window: HWND) -> Result<(), String> {
    control(
        window,
        w!("STATIC"),
        "PathWarp 设置",
        201,
        WINDOW_STYLE::default(),
    )?;
    control(
        window,
        w!("STATIC"),
        "悬浮层主题",
        202,
        WINDOW_STYLE::default(),
    )?;
    for (id, text) in [(AUTO, "跟随系统"), (LIGHT, "浅色"), (DARK, "深色")] {
        control(
            window,
            w!("BUTTON"),
            text,
            id,
            WINDOW_STYLE(BS_AUTORADIOBUTTON as u32)
                | WS_TABSTOP
                | if id == AUTO {
                    WS_GROUP
                } else {
                    WINDOW_STYLE::default()
                },
        )?;
    }
    for id in [THEME_STATUS, STARTUP_STATUS] {
        control(
            window,
            w!("EDIT"),
            "",
            id,
            WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32)
                | WS_VSCROLL
                | WS_TABSTOP,
        )?;
    }
    control(
        window,
        w!("BUTTON"),
        "重试保存",
        RETRY,
        WS_TABSTOP | WS_GROUP,
    )?;
    control(
        window,
        w!("STATIC"),
        "当前用户登录启动项",
        203,
        WINDOW_STYLE::default(),
    )?;
    control(
        window,
        w!("BUTTON"),
        "添加此副本的登录启动项",
        USE_CURRENT,
        WS_TABSTOP | WS_GROUP,
    )?;
    control(window, w!("BUTTON"), "移除登录启动项", REMOVE, WS_TABSTOP)?;
    control(window, w!("BUTTON"), "刷新登记", REFRESH, WS_TABSTOP)?;
    control(
        window,
        w!("STATIC"),
        "",
        FOCUS_STATUS,
        WINDOW_STYLE::default(),
    )?;
    Ok(())
}

fn item(window: HWND, id: i32) -> Option<HWND> {
    unsafe { GetDlgItem(Some(window), id) }.ok()
}
fn set_text(window: HWND, id: i32, text: &str) {
    if let Some(item) = item(window, id) {
        let text = wide(text);
        unsafe {
            let _ = SetWindowTextW(item, PCWSTR(text.as_ptr()));
        }
    }
}

fn sync_view(shared: &Shared, window: HWND) {
    shared.present(|view| render_view(window, view));
}

fn render_view(window: HWND, view: &View) {
    set_text(window, THEME_STATUS, &view.theme_status);
    set_text(window, STARTUP_STATUS, &view.startup_status);
    set_text(window, USE_CURRENT, &view.use_label);
    for (id, preference) in [
        (AUTO, ThemePreference::Auto),
        (LIGHT, ThemePreference::Light),
        (DARK, ThemePreference::Dark),
    ] {
        if let Some(item) = item(window, id) {
            unsafe {
                SendMessageW(
                    item,
                    BM_SETCHECK,
                    Some(WPARAM(usize::from(view.theme == preference))),
                    None,
                );
            }
        }
    }
    for (id, enabled) in [
        (RETRY, view.retry_save),
        (USE_CURRENT, view.can_use),
        (REMOVE, view.can_remove),
    ] {
        if let Some(item) = item(window, id) {
            unsafe {
                let _ = EnableWindow(item, enabled);
            }
        }
    }
}

fn layout(window: HWND, font: &mut HFONT) {
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let scale = |value: i32| i32::try_from(i64::from(value) * i64::from(dpi) / 96).unwrap_or(value);
    let mut rect = RECT::default();
    if unsafe { GetClientRect(window, &mut rect) }.is_err() {
        return;
    }
    let width = (rect.right - rect.left) * 96 / i32::try_from(dpi).unwrap_or(96);
    let height = (rect.bottom - rect.top) * 96 / i32::try_from(dpi).unwrap_or(96);
    let bottom = height.max(440);
    let positions = [
        (201, 24, 18, width - 48, 28),
        (202, 24, 58, width - 48, 22),
        (AUTO, 24, 84, 140, 28),
        (LIGHT, 172, 84, 100, 28),
        (DARK, 280, 84, 100, 28),
        (THEME_STATUS, 24, 122, width - 168, 94),
        (RETRY, width - 132, 122, 108, 30),
        (203, 24, 230, width - 48, 24),
        (STARTUP_STATUS, 24, 262, width - 48, bottom - 362),
        (USE_CURRENT, 24, bottom - 90, 258, 32),
        (REMOVE, 292, bottom - 90, 152, 32),
        (REFRESH, width - 132, bottom - 90, 108, 32),
        (FOCUS_STATUS, 24, bottom - 44, width - 48, 36),
    ];
    let new_font = unsafe {
        CreateFontW(
            -scale(14),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0.into(),
            w!("Segoe UI"),
        )
    };
    for (id, x, y, width, height) in positions {
        if let Some(item) = item(window, id) {
            unsafe {
                let _ = SetWindowPos(
                    item,
                    None,
                    scale(x),
                    scale(y),
                    scale(width.max(1)),
                    scale(height.max(1)),
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
                SendMessageW(
                    item,
                    WM_SETFONT,
                    Some(WPARAM(new_font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
        }
    }
    if !font.is_invalid() {
        unsafe {
            let _ = DeleteObject((*font).into());
        }
    }
    *font = new_font;
}

fn open_settings(shared: &Arc<Shared>, font: &mut HFONT) {
    shared.begin_settings_open();
    let raw = shared.settings.load(Ordering::Acquire);
    let window = if raw != 0
        && unsafe {
            IsWindow(Some(hwnd(raw))).as_bool()
                && GetWindowLongPtrW(hwnd(raw), GWLP_USERDATA) == Arc::as_ptr(shared) as isize
        } {
        hwnd(raw)
    } else {
        // Class registration survives closing the window.
        let class = w!("PathWarp.Settings");
        let result = create_window(shared, class, w!("PathWarp 设置"), true);
        match result {
            Ok(window) => {
                shared.settings.store(window.0 as isize, Ordering::Release);
                if let Err(error) = create_controls(window) {
                    unsafe {
                        let _ = DestroyWindow(window);
                    }
                    shared.settings.store(0, Ordering::Release);
                    shared.send(Command::Diagnostic(Some(format!(
                        "设置窗口无法创建控件：{error}"
                    ))));
                    shared.settings_closed();
                    return;
                }
                let dpi = unsafe { GetDpiForWindow(window) }.max(96);
                let mut info = MONITORINFO {
                    cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or_default(),
                    ..Default::default()
                };
                if unsafe {
                    GetMonitorInfoW(
                        MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
                        &mut info,
                    )
                }
                .as_bool()
                {
                    let width = i32::try_from(640 * dpi / 96)
                        .unwrap_or(640)
                        .min(info.rcWork.right - info.rcWork.left);
                    let height = i32::try_from(640 * dpi / 96)
                        .unwrap_or(640)
                        .min(info.rcWork.bottom - info.rcWork.top);
                    unsafe {
                        let _ = SetWindowPos(
                            window,
                            None,
                            info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2,
                            info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 2,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        );
                    }
                }
                window
            }
            Err(error) => {
                shared.send(Command::Diagnostic(Some(format!(
                    "设置窗口无法打开：{error}"
                ))));
                shared.settings_closed();
                return;
            }
        }
    };
    layout(window, font);
    sync_view(shared, window);
    unsafe {
        // Showing/restoring does not itself request foreground. Exactly one
        // ordinary activation request follows this explicit open command.
        let _ = ShowWindow(
            window,
            if IsIconic(window).as_bool() {
                SW_SHOWNOACTIVATE
            } else {
                SW_SHOWNA
            },
        );
    }
    let focused = super::window_ext::activate(window.0 as isize);
    log::debug!(
        "settings open: hwnd={} confirmed_foreground={focused}",
        window.0 as isize
    );
    set_text(
        window,
        FOCUS_STATUS,
        if focused {
            "设置已取得焦点。关闭后应用继续留在托盘。"
        } else {
            "设置已打开，尚未确认取得焦点；请点击此窗口。"
        },
    );
    if focused && let Some(control) = item(window, AUTO) {
        unsafe {
            let _ = SetFocus(Some(control));
        }
    }
    shared.finish_settings_open(focused);
}

fn menu(shared: &Arc<Shared>, owner: HWND, font: &mut HFONT, mut point: POINT) {
    shared.menu_active.store(true, Ordering::Release);
    shared.interact();
    let selected = unsafe {
        match CreatePopupMenu() {
            Ok(menu) => {
                let _ = AppendMenuW(menu, MF_STRING, 1, w!("设置…"));
                if point.x == -1 && point.y == -1 {
                    let _ = GetCursorPos(&mut point);
                }
                let _ = SetForegroundWindow(owner);
                let selected = TrackPopupMenuEx(
                    menu,
                    (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                    point.x,
                    point.y,
                    owner,
                    None,
                )
                .0;
                let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
                let _ = DestroyMenu(menu);
                selected
            }
            Err(error) => {
                shared.send(Command::Diagnostic(Some(format!(
                    "托盘菜单不可用：{error}"
                ))));
                0
            }
        }
    };
    shared.menu_active.store(false, Ordering::Release);
    if selected == 1 {
        open_settings(shared, font);
    } else {
        let _ = notify(owner, NIM_SETFOCUS);
    }
    shared.interact();
}

fn run(shared: Arc<Shared>, owner: HWND, events: mpsc::Receiver<NativeEvent>) {
    restore_tray(&shared, owner);
    let mut font = HFONT::default();
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        match message.message {
            OPEN => open_settings(&shared, &mut font),
            SYNC => {
                let raw = shared.settings.load(Ordering::Acquire);
                if raw != 0 {
                    sync_view(&shared, hwnd(raw));
                }
            }
            _ => {
                let raw = shared.settings.load(Ordering::Acquire);
                if raw == 0 || !unsafe { IsDialogMessageW(hwnd(raw), &message).as_bool() } {
                    unsafe {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        }
        for event in events.try_iter() {
            match event {
                NativeEvent::Menu(point) => menu(&shared, owner, &mut font, point),
                NativeEvent::Layout => {
                    let raw = shared.settings.load(Ordering::Acquire);
                    if raw != 0 {
                        layout(hwnd(raw), &mut font);
                    }
                }
                NativeEvent::RecreateTray => restore_tray(&shared, owner),
            }
        }
    }
    let _ = notify(owner, NIM_DELETE);
    let raw = shared.settings.swap(0, Ordering::AcqRel);
    unsafe {
        if raw != 0 {
            let _ = DestroyWindow(hwnd(raw));
        }
        let _ = DestroyWindow(owner);
        if !font.is_invalid() {
            let _ = DeleteObject(font.into());
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::ConfigStore;
    use crate::startup::{Backend, RawRegistration, Registration, Target};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct RegistrationBackend {
        raw: Rc<RefCell<RawRegistration>>,
        writes: Rc<Cell<usize>>,
    }

    impl Backend for RegistrationBackend {
        fn lock(&self) -> Result<Box<dyn std::any::Any>, String> {
            Ok(Box::new(()))
        }
        fn read(&self) -> Result<RawRegistration, String> {
            Ok(self.raw.borrow().clone())
        }
        fn write(&self, value: &RawRegistration) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            *self.raw.borrow_mut() = value.clone();
            Ok(())
        }
        fn validate_target(&self, _: &Target) -> Result<(), String> {
            Ok(())
        }
    }

    fn desktop(view: View) -> Desktop {
        let (commands, receiver) = mpsc::channel();
        let (native, _) = mpsc::channel();
        let shared = Arc::new(Shared {
            ctx: egui::Context::default(),
            commands,
            native,
            thread_id: 0,
            overlay: AtomicIsize::new(0),
            settings: AtomicIsize::new(0),
            owner: AtomicIsize::new(0),
            menu_active: AtomicBool::new(false),
            window_active: AtomicBool::new(false),
            open_pending: AtomicBool::new(false),
            cancellation: Mutex::new(None),
            shell_message: 0,
            view: Mutex::new(view),
            rendered_revision: Mutex::new(None),
        });
        Desktop {
            shared,
            receiver,
            thread: None,
        }
    }

    #[test]
    fn startup_clicks_before_native_sync_preserve_the_other_registration() {
        for action in [Action::UseCurrent, Action::Remove] {
            let current = Target {
                executable: r"C:\current\PathWarp.exe".into(),
                storage: r"C:\current\store".into(),
            };
            let current_raw = RawRegistration::command(&current.command().unwrap());
            let raw = Rc::new(RefCell::new(if action == Action::UseCurrent {
                RawRegistration::Absent
            } else {
                current_raw.clone()
            }));
            let writes = Rc::new(Cell::new(0));
            let mut registration = Registration::new(
                RegistrationBackend {
                    raw: Rc::clone(&raw),
                    writes: Rc::clone(&writes),
                },
                Ok(current),
            );
            let (config, _) = ConfigStore::open(None);
            let desktop = desktop(View::new(ThemePreference::Auto, &config, &registration));
            desktop.shared.present(|view| {
                if action == Action::UseCurrent {
                    assert_eq!(view.use_label, "添加此副本的登录启动项");
                    assert!(view.can_use);
                } else {
                    assert!(view.can_remove);
                }
            });
            let other = Target {
                executable: r"C:\other\PathWarp.exe".into(),
                storage: r"C:\other\store".into(),
            };
            let other_raw = RawRegistration::command(&other.command().unwrap());
            *raw.borrow_mut() = other_raw.clone();
            registration.refresh();
            // Publish the refreshed state without processing the native SYNC.
            desktop.update(View::new(ThemePreference::Auto, &config, &registration));
            let Some(Command::Startup(action, revision)) = desktop.shared.startup_command(action)
            else {
                panic!("the previously displayed action should retain its revision");
            };
            registration.execute(action, revision);
            assert_eq!(
                writes.get(),
                0,
                "a click on the old controls must not change another copy's registration"
            );
            assert_eq!(*raw.borrow(), other_raw);

            desktop.shared.present(|view| {
                assert_eq!(view.use_label, "改用当前副本（共享登记）");
                assert!(view.can_use && view.can_remove);
            });
            let Some(Command::Startup(action, revision)) = desktop.shared.startup_command(action)
            else {
                panic!("the refreshed controls should allow an explicit action");
            };
            registration.execute(action, revision);
            assert_eq!(writes.get(), 1);
            assert_eq!(
                *raw.borrow(),
                if action == Action::UseCurrent {
                    current_raw
                } else {
                    RawRegistration::Absent
                }
            );
        }
    }

    #[test]
    fn startup_actions_wait_for_a_complete_native_presentation() {
        let desktop = desktop(View {
            theme: ThemePreference::Auto,
            theme_status: String::new(),
            retry_save: false,
            startup_status: String::new(),
            use_label: String::new(),
            can_use: true,
            can_remove: true,
            revision: 1,
        });
        assert!(desktop.shared.startup_command(Action::UseCurrent).is_none());
        desktop.shared.present(|view| {
            assert!(desktop.shared.startup_command(Action::UseCurrent).is_none());
            let mut pending = view.clone();
            pending.revision = 2;
            desktop.update(pending);
        });
        assert!(matches!(
            desktop.shared.startup_command(Action::UseCurrent),
            Some(Command::Startup(Action::UseCurrent, 1))
        ));
        desktop.shared.present(|view| {
            assert_eq!(view.revision, 2);
            assert!(desktop.shared.startup_command(Action::Remove).is_none());
        });
        assert!(matches!(
            desktop.shared.startup_command(Action::Remove),
            Some(Command::Startup(Action::Remove, 2))
        ));
        desktop.shared.settings_closed();
        assert!(desktop.shared.startup_command(Action::Remove).is_none());
    }

    #[test]
    fn unfocused_settings_open_does_not_end_the_pending_interaction() {
        let desktop = desktop(View {
            theme: ThemePreference::Auto,
            theme_status: String::new(),
            retry_save: false,
            startup_status: String::new(),
            use_label: String::new(),
            can_use: false,
            can_remove: false,
            revision: 0,
        });
        let shared = &desktop.shared;
        let receiver = &desktop.receiver;
        shared.begin_settings_open();
        shared.finish_settings_open(false);
        assert!(
            matches!(receiver.try_iter().last(), Some(Command::Interaction(true))),
            "an unfocused open must stay pending until click/close, without resuming search"
        );
        shared.settings_focus_changed(true);
        shared.settings_focus_changed(false);
        assert!(matches!(
            receiver.try_iter().last(),
            Some(Command::Interaction(false))
        ));
        shared.begin_settings_open();
        shared.finish_settings_open(false);
        shared.settings_closed();
        assert!(matches!(
            receiver.try_iter().last(),
            Some(Command::Interaction(false))
        ));
    }
}
