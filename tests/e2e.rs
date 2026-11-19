//! Windows 端到端测试：启动真实 PathWarp 进程 + 真实 `IFileOpenDialog`，
//! 用 Win32 探针断言显隐/停靠/前台行为。
//!
//! 这些测试需要交互式桌面（会真实移动鼠标、抢占前台），默认 `#[ignore]`，
//! 通过 `just e2e` 串行运行。被测进程与对话框宿主进程都在测试结束时清理。
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use uiautomation::{UIAutomation, controls::ControlType, patterns::UIValuePattern};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
    SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput, SetFocus,
    VIRTUAL_KEY, VK_CONTROL, VK_G,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GUITHREADINFO, GWL_EXSTYLE, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    SWP_NOACTIVATE, SWP_NOSENDCHANGING, SWP_NOZORDER, SetCursorPos, SetForegroundWindow,
    SetWindowPos, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW};
use windows::core::BOOL;

// ---------- 进程管理 ----------

/// 结束时强杀子进程的守卫。
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_pathwarp() -> (Proc, u32) {
    let child = Command::new(env!("CARGO_BIN_EXE_PathWarp"))
        .env("RUST_LOG", "debug")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn PathWarp");
    let pid = child.id();
    (Proc(child), pid)
}

/// 文件对话框宿主：stdin 行协议控制真实 `IFileOpenDialog`。
struct DialogHost {
    /// 仅作存活守卫：drop 时兜底强杀宿主进程。
    _proc: Proc,
    stdin: ChildStdin,
    pid: u32,
}

impl DialogHost {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dialog_host"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn dialog_host");
        let pid = child.id();
        let stdin = child.stdin.take().expect("dialog_host stdin");
        let stdout = child.stdout.take().expect("dialog_host stdout");

        // 等 READY，确认进程起来了。
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read READY");
        assert_eq!(line.trim(), "READY", "dialog_host handshake failed");

        Self {
            _proc: Proc(child),
            stdin,
            pid,
        }
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{cmd}").expect("write to dialog_host stdin");
        self.stdin.flush().expect("flush dialog_host stdin");
    }

    /// 打开对话框并等它成为可见前台窗口，返回对话框 HWND。
    ///
    /// Windows 前台锁定规则只保证新进程的首个窗口能自动拿到前台；宿主复用后
    /// 再开的对话框会停在后台。测试环境（如 IDE 内跑）里其它进程会抢前台，
    /// 故用 `AttachThreadInput` 经典手法强制把对话框提到前台。
    fn open_dialog(&mut self) -> isize {
        self.send("open");
        let hwnd = wait_for(Duration::from_secs(8), || dialog_of_pid(self.pid))
            .expect("file dialog window never appeared");

        for _ in 0..5 {
            if wait_until(Duration::from_millis(600), || is_foreground(hwnd)) {
                return hwnd;
            }
            force_foreground(hwnd);
        }
        let fg = unsafe { GetForegroundWindow().0 as isize };
        panic!(
            "file dialog {hwnd} did not become foreground; foreground={fg} class='{}' title='{}'",
            class_of(fg),
            title_of(fg),
        );
    }

    fn close_dialog(&mut self) {
        self.send("close");
        let closed = wait_until(Duration::from_secs(5), || dialog_of_pid(self.pid).is_none());
        assert!(closed, "file dialog did not close");
    }
}

impl Drop for DialogHost {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "exit");
    }
}

// ---------- Win32 探针 ----------

fn windows_of_pid(pid: u32) -> Vec<isize> {
    struct Ctx {
        pid: u32,
        hwnds: Vec<isize>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
        let mut win_pid = 0u32;
        unsafe {
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
        }
        if win_pid == ctx.pid {
            ctx.hwnds.push(hwnd.0 as isize);
        }
        BOOL(1)
    }

    let mut ctx = Ctx {
        pid,
        hwnds: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM((&raw mut ctx) as isize));
    }
    ctx.hwnds
}

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

fn class_of(raw: isize) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd_of(raw), &mut buf) };
    usize::try_from(len)
        .map(|n| String::from_utf16_lossy(&buf[..n]))
        .unwrap_or_default()
}

fn rect_of(raw: isize) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd_of(raw), &mut rect);
    }
    rect
}

struct DpiContext(DPI_AWARENESS_CONTEXT);

impl DpiContext {
    fn physical_pixels() -> Self {
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        assert!(
            !previous.is_invalid(),
            "could not use physical desktop coordinates"
        );
        Self(previous)
    }
}

impl Drop for DpiContext {
    fn drop(&mut self) {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(self.0);
        }
    }
}

#[derive(Clone, Copy)]
struct Monitor {
    handle: HMONITOR,
    work: RECT,
}

fn desktop_monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &raw mut info) }.as_bool() {
            let monitors = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
            monitors.push(Monitor {
                handle: monitor,
                work: info.rcWork,
            });
        }
        BOOL(1)
    }
    let mut monitors = Vec::<Monitor>::new();
    assert!(
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(collect),
                LPARAM((&raw mut monitors) as isize),
            )
        }
        .as_bool()
    );
    assert!(!monitors.is_empty(), "no desktop monitors available");
    monitors
}

fn visual_rect_of(raw: isize) -> RECT {
    let mut rect = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd_of(raw),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut rect).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if result.is_ok() { rect } else { rect_of(raw) }
}

/// Arrange only the owned fixture. Re-measure DWM frame margins after a DPI transition.
fn move_dialog(raw: isize, x: i32, y: i32, width: i32, height: i32) {
    for _ in 0..3 {
        let outer = rect_of(raw);
        let visual = visual_rect_of(raw);
        unsafe {
            SetWindowPos(
                hwnd_of(raw),
                None,
                x - (visual.left - outer.left),
                y - (visual.top - outer.top),
                width + (outer.right - outer.left) - (visual.right - visual.left),
                height + (outer.bottom - outer.top) - (visual.bottom - visual.top),
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSENDCHANGING,
            )
            .unwrap();
        }
        if wait_until(Duration::from_millis(500), || {
            let actual = visual_rect_of(raw);
            (
                actual.left,
                actual.top,
                actual.right - actual.left,
                actual.bottom - actual.top,
            ) == (x, y, width, height)
        }) {
            return;
        }
    }
    panic!(
        "owned dialog did not reach requested physical rectangle: {:?}",
        visual_rect_of(raw)
    );
}

fn inside(rect: &RECT, area: &RECT) -> bool {
    rect.right > rect.left
        && rect.bottom > rect.top
        && rect.left >= area.left
        && rect.top >= area.top
        && rect.right <= area.right
        && rect.bottom <= area.bottom
}

fn title_of(raw: isize) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd_of(raw), &mut buf) };
    usize::try_from(len)
        .map(|n| String::from_utf16_lossy(&buf[..n]))
        .unwrap_or_default()
}

fn is_visible(raw: isize) -> bool {
    unsafe { IsWindowVisible(hwnd_of(raw)).as_bool() }
}

fn is_foreground(raw: isize) -> bool {
    unsafe { GetForegroundWindow().0 as isize == raw }
}

fn native_focus(raw: isize) -> Option<isize> {
    let thread = unsafe { GetWindowThreadProcessId(hwnd_of(raw), None) };
    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(thread, &mut info) }.ok()?;
    Some(info.hwndFocus.0 as isize)
}

/// 判定窗口矩形是否落在“正常桌面区域”（排除 -32000 停靠点与零尺寸）。
fn is_onscreen(rect: &RECT) -> bool {
    rect.right > rect.left && rect.bottom > rect.top && rect.left > -10000 && rect.top > -10000
}

/// 本进程的可见文件对话框（`#32770`）。
fn dialog_of_pid(pid: u32) -> Option<isize> {
    windows_of_pid(pid)
        .into_iter()
        .find(|&h| is_visible(h) && class_of(h) == "#32770")
}

/// winit 0.30 给应用主窗口注册的窗口类名（区别于 `Winit Thread Event Target`
/// / `wgpu Device Class` 等常驻辅助窗口，那些不能作为悬浮条判定依据）。
const OVERLAY_WINDOW_CLASS: &str = "Window Class";

/// PathWarp 的悬浮条窗口（不论显隐）。
fn overlay_of_pid(pid: u32) -> Option<isize> {
    windows_of_pid(pid)
        .into_iter()
        .find(|&h| class_of(h) == OVERLAY_WINDOW_CLASS)
}

/// 悬浮条当前「可见且在屏幕内」时返回其矩形。
fn overlay_onscreen(pid: u32) -> Option<(isize, RECT)> {
    let h = overlay_of_pid(pid)?;
    if !is_visible(h) {
        return None;
    }
    let rect = rect_of(h);
    is_onscreen(&rect).then_some((h, rect))
}

/// 移动鼠标到 (x, y) 并单击左键。
fn click_at(x: i32, y: i32) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
    std::thread::sleep(Duration::from_millis(60));

    let make_input = |flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        make_input(MOUSEEVENTF_LEFTDOWN),
        make_input(MOUSEEVENTF_LEFTUP),
    ];
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

/// 用 `AttachThreadInput` 手法把窗口强制提到前台（绕过前台锁定），
/// 测试环境中别的进程（IDE 等）抢占前台时用它稳住目标对话框。
fn force_foreground(target: isize) {
    let target = hwnd_of(target);
    unsafe {
        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        let fg_thread = GetWindowThreadProcessId(fg, Some(&mut fg_pid));
        let our_thread = GetCurrentThreadId();

        let attached = AttachThreadInput(our_thread, fg_thread, true).as_bool();
        let _ = BringWindowToTop(target);
        let _ = SetForegroundWindow(target);
        let _ = SetFocus(Some(target));
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }
}

fn center_of(rect: &RECT) -> POINT {
    POINT {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    }
}

fn keyboard_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

// Input simulation is confined to the interactive test driver, never the app.
fn press_ctrl_g() {
    let inputs = [
        keyboard_input(VK_CONTROL, 0, KEYBD_EVENT_FLAGS::default()),
        keyboard_input(VK_G, 0, KEYBD_EVENT_FLAGS::default()),
        keyboard_input(VK_G, 0, KEYEVENTF_KEYUP),
        keyboard_input(VK_CONTROL, 0, KEYEVENTF_KEYUP),
    ];
    assert_eq!(unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) }, 4);
}

fn type_text(text: &str) {
    let inputs: Vec<_> = text
        .encode_utf16()
        .flat_map(|scan| {
            [
                keyboard_input(VIRTUAL_KEY(0), scan, KEYEVENTF_UNICODE),
                keyboard_input(VIRTUAL_KEY(0), scan, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
            ]
        })
        .collect();
    assert_eq!(
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize,
        inputs.len()
    );
}

fn focused_editor(automation: &UIAutomation, app_pid: u32) -> Option<uiautomation::UIElement> {
    let element = automation.get_focused_element().ok()?;
    (element.get_process_id().ok()? == app_pid
        && element.get_control_type().ok()? == ControlType::Edit
        && element.has_keyboard_focus().ok()?)
    .then_some(element)
}

fn assert_shortcut_focus(automation: &UIAutomation, app_pid: u32, overlay: isize) {
    press_ctrl_g();
    let ready = wait_until(Duration::from_secs(3), || {
        is_foreground(overlay) && focused_editor(automation, app_pid).is_some()
    });
    assert!(
        ready,
        "Ctrl+G did not give the actual search editor keyboard focus; foreground={} expected_overlay={overlay}",
        unsafe { GetForegroundWindow().0 as isize },
    );
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn wait_for<T>(timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(value) = probe() {
            return Some(value);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

// ---------- 测试用例 ----------

/// 诊断辅助：探明点击悬浮条时它为何被激活——运行时它到底有没有 WS_EX_NOACTIVATE，
/// 点击后前台落到谁身上。
#[test]
#[ignore = "diagnostic helper; run manually"]
fn diagnose_overlay_activation() {
    let ex_flags = |raw: isize| -> String {
        let ex = unsafe { GetWindowLongPtrW(hwnd_of(raw), GWL_EXSTYLE) } as u32;
        let mut names = Vec::new();
        if ex & WS_EX_NOACTIVATE.0 != 0 {
            names.push("NOACTIVATE");
        }
        if ex & WS_EX_TOOLWINDOW.0 != 0 {
            names.push("TOOLWINDOW");
        }
        if ex & WS_EX_TOPMOST.0 != 0 {
            names.push("TOPMOST");
        }
        format!("0x{ex:08X} [{}]", names.join("|"))
    };

    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();
    let dialog = host.open_dialog();
    let (overlay, rect) = wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid))
        .expect("overlay did not dock");

    eprintln!("overlay={overlay} dialog={dialog}");
    eprintln!("overlay ex-style BEFORE click: {}", ex_flags(overlay));
    eprintln!("foreground BEFORE click: {}", unsafe {
        GetForegroundWindow().0 as isize
    });

    let center = center_of(&rect);
    click_at(center.x, center.y);

    for i in 0..6 {
        std::thread::sleep(Duration::from_millis(50));
        let fg = unsafe { GetForegroundWindow().0 as isize };
        let tag = if fg == overlay {
            "OVERLAY"
        } else if fg == dialog {
            "dialog"
        } else {
            "other"
        };
        eprintln!(
            "  t+{}ms fg={fg} ({tag}) class='{}'",
            (i + 1) * 50,
            class_of(fg)
        );
    }
    eprintln!("overlay ex-style AFTER click: {}", ex_flags(overlay));
}

/// 诊断辅助：dump 被测进程在「空闲 / 对话框打开 / 对话框关闭」三个阶段的窗口清单。
/// 排查探针误匹配（winit/wgpu 辅助窗口）时手动运行。
#[test]
#[ignore = "diagnostic helper; run manually"]
fn dump_windows() {
    let dump = |pid: u32, label: &str| {
        println!("--- {label} (pid={pid}) ---");
        for h in windows_of_pid(pid) {
            let r = rect_of(h);
            println!(
                "hwnd={h:>10} visible={:<5} class='{}' rect=({}, {}) {}x{}",
                is_visible(h),
                class_of(h),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
            );
        }
    };

    let (_app, app_pid) = spawn_pathwarp();
    std::thread::sleep(Duration::from_secs(2));
    dump(app_pid, "startup");

    let mut host = DialogHost::spawn();
    host.open_dialog();
    std::thread::sleep(Duration::from_secs(1));
    dump(app_pid, "dialog open");

    host.close_dialog();
    std::thread::sleep(Duration::from_secs(1));
    dump(app_pid, "dialog closed");
}

/// 回归（核心）：点击悬浮条后，① 悬浮窗获得真实前台焦点，② 原文件对话框仍被
/// 当前会话跟踪，③ 悬浮条保持停靠可见。这个行为让 egui `TextEdit` 能接收系统编辑
/// 事件和闪烁 caret。
///
/// 需要干净桌面：其它会抢占前台的工具（如 Listary）会改变点击后的焦点归属，导致
/// 断言不稳定。运行前请关闭此类工具。
#[test]
#[ignore = "requires a clean interactive desktop (close Listary etc.); run via `just e2e`"]
fn clicking_overlay_activates_it_and_keeps_it_docked() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();

    let dialog = host.open_dialog();
    let (overlay_hwnd, overlay_rect) =
        wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid))
            .expect("overlay did not dock after dialog opened");

    let center = center_of(&overlay_rect);
    click_at(center.x, center.y);

    // 给足任何潜在的去抖/隐藏窗口时间，再断言状态已稳定。
    std::thread::sleep(Duration::from_millis(600));

    let fg = unsafe { GetForegroundWindow().0 as isize };
    assert_eq!(
        fg, overlay_hwnd,
        "overlay window did not become the foreground window after click"
    );
    assert!(
        !is_foreground(dialog),
        "file dialog should yield focus to the overlay"
    );
    assert!(
        overlay_onscreen(app_pid).is_some(),
        "overlay disappeared after being clicked"
    );
}

/// 回归：点击悬浮条后（用户报告触发点），关闭并重开对话框，悬浮条必须再次出现。
/// 旧实现里点击误激活悬浮窗 + 隐藏后事件循环停摆，导致“永远不再出现”。
#[test]
#[ignore = "requires interactive desktop; run via `just e2e`"]
fn overlay_recovers_after_click_and_reopen() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();

    host.open_dialog();
    let (_, rect) = wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid))
        .expect("overlay did not dock on first dialog open");

    // 触发用户报告的动作：点击悬浮条。
    let center = center_of(&rect);
    click_at(center.x, center.y);
    std::thread::sleep(Duration::from_millis(400));

    host.close_dialog();
    host.open_dialog();

    assert!(
        wait_until(Duration::from_secs(5), || overlay_onscreen(app_pid)
            .is_some()),
        "overlay never reappeared after click + dialog reopen (permanent-disappearance regression)"
    );
}

/// 回归：单纯的关闭→重开也必须让悬浮条再次出现（事件循环不得停摆）。
#[test]
#[ignore = "requires interactive desktop; run via `just e2e`"]
fn overlay_survives_dialog_reopen() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();

    host.open_dialog();
    assert!(
        wait_until(Duration::from_secs(5), || overlay_onscreen(app_pid)
            .is_some()),
        "overlay did not dock on first dialog open"
    );

    host.close_dialog();
    assert!(
        wait_until(Duration::from_secs(3), || overlay_onscreen(app_pid)
            .is_none()),
        "overlay did not hide after dialog closed"
    );

    host.open_dialog();
    assert!(
        wait_until(Duration::from_secs(5), || overlay_onscreen(app_pid)
            .is_some()),
        "overlay never reappeared after dialog reopen (event loop starvation regression)"
    );
}

#[test]
#[ignore = "moves owned windows across all attached monitors; run via `just e2e`"]
fn overlay_stays_in_each_monitor_work_area_at_dialog_edges() {
    let _dpi = DpiContext::physical_pixels();
    let monitors = desktop_monitors();
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();
    let dialog = host.open_dialog();
    wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid)).unwrap();

    for (index, monitor) in monitors.iter().enumerate() {
        let work = monitor.work;
        let work_width = work.right - work.left;
        let work_height = work.bottom - work.top;
        assert!(
            work_width > 100 && work_height > 100,
            "fixture needs a usable work area"
        );
        let width = 600.min(work_width - 40);
        let height = 400.min(work_height / 2);
        let cases = [
            ("normal", work.left + 20, work.top + 20, width, height),
            (
                "bottom",
                work.right - width,
                work.bottom - height,
                width,
                height,
            ),
            (
                "wide",
                work.left - 40,
                work.top + 20,
                work_width + 80,
                height,
            ),
            ("left", work.left - 40, work.top + 20, width, height),
            (
                "tight",
                work.left + 20,
                work.top + 12,
                width,
                work_height - 24,
            ),
            ("cover", work.left + 20, work.top, width, work_height),
        ];
        for (label, x, y, width, height) in cases {
            move_dialog(dialog, x, y, width, height);
            assert_eq!(
                unsafe { MonitorFromWindow(hwnd_of(dialog), MONITOR_DEFAULTTONEAREST) },
                monitor.handle,
                "fixture moved to a different monitor"
            );
            let visual = visual_rect_of(dialog);
            let dpi = unsafe { GetDpiForWindow(hwnd_of(dialog)) };
            assert!(dpi > 0);
            let full_height = i32::try_from(140 * u64::from(dpi) / 96).unwrap();
            let expected_width = width.min(work_width);
            let expected_x = x.clamp(work.left, work.right - expected_width);
            let sample = wait_for(Duration::from_secs(5), || {
                let (_, overlay) = overlay_onscreen(app_pid)?;
                let actual_height = overlay.bottom - overlay.top;
                let placement_matches = match label {
                    "normal" if visual.bottom + full_height <= work.bottom => {
                        overlay.top == visual.bottom && actual_height == full_height
                    }
                    "bottom" => {
                        overlay.bottom == visual.top
                            && actual_height == full_height.min(visual.top - work.top)
                    }
                    "tight" => {
                        actual_height == 12
                            && (overlay.top == visual.bottom || overlay.bottom == visual.top)
                    }
                    "cover" => actual_height == full_height.min(work_height),
                    _ => true,
                };
                (inside(&overlay, &work)
                    && overlay.left == expected_x
                    && overlay.right - overlay.left == expected_width
                    && placement_matches)
                    .then_some(overlay)
            });
            assert!(
                sample.is_some(),
                "monitor {index} {label}: work={work:?} dialog={visual:?} overlay={:?}",
                overlay_onscreen(app_pid).map(|(_, rect)| rect)
            );
            eprintln!(
                "monitor={index} case={label} dpi={dpi} work={work:?} overlay={:?}",
                sample.unwrap()
            );
        }
    }
}

#[test]
#[ignore = "requires interactive keyboard/mouse input; run via `just e2e`"]
fn ctrl_g_focuses_editor_repeats_and_leaves_unrelated_windows_alone() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();
    host.open_dialog();
    let (overlay, before) = wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid))
        .expect("overlay did not dock");
    let automation = UIAutomation::new().unwrap();
    // Start the app's accesskit provider before checking actual editor focus.
    automation.element_from_handle(overlay.into()).unwrap();
    assert_shortcut_focus(&automation, app_pid, overlay);
    type_text("ctrlg");
    assert!(wait_until(Duration::from_secs(3), || focused_editor(
        &automation,
        app_pid
    )
    .and_then(|edit| edit.get_pattern::<UIValuePattern>().ok())
    .and_then(|value| value.get_value().ok())
    .as_deref()
        == Some("ctrlg")));
    assert_shortcut_focus(&automation, app_pid, overlay);
    assert_shortcut_focus(&automation, app_pid, overlay);
    let after = rect_of(overlay);
    assert_eq!(
        (before.left, before.top, before.right, before.bottom),
        (after.left, after.top, after.right, after.bottom)
    );

    // An owned plain window supplies independent proof that G was delivered
    // through the hook chain, not merely that PathWarp stayed in the background.
    host.send("probe");
    let probe = wait_for(Duration::from_secs(3), || {
        windows_of_pid(host.pid)
            .into_iter()
            .find(|&window| class_of(window) == "PathWarpShortcutProbe")
    })
    .unwrap();
    force_foreground(probe); // Fixture setup only; no assistance after the chord.
    assert!(wait_until(Duration::from_secs(2), || is_foreground(probe)));
    press_ctrl_g();
    assert!(
        wait_until(Duration::from_secs(3), || title_of(probe)
            == "PathWarp shortcut probe: 1"),
        "unrelated Ctrl+G was not delivered once; title={} foreground={} probe={probe} native_focus={:?}",
        title_of(probe),
        unsafe { GetForegroundWindow().0 as isize },
        native_focus(probe),
    );
    assert!(is_foreground(probe));
    host.close_dialog();
    assert!(wait_until(Duration::from_secs(3), || overlay_onscreen(
        app_pid
    )
    .is_none()));
    press_ctrl_g();
    assert!(wait_until(Duration::from_secs(3), || title_of(probe)
        == "PathWarp shortcut probe: 2"));
    assert!(is_foreground(probe));
    assert!(overlay_onscreen(app_pid).is_none());

    host.open_dialog();
    wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid)).unwrap();
    assert_shortcut_focus(&automation, app_pid, overlay);
}

#[test]
#[ignore = "requires interactive keyboard input; run via `just e2e`"]
fn ctrl_g_reports_denied_activation_then_recovers_normally() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();
    let dialog = host.open_dialog();
    let (overlay, _) = wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid)).unwrap();
    let automation = UIAutomation::new().unwrap();
    let root = automation.element_from_handle(overlay.into()).unwrap();
    host.send("lock");
    assert!(wait_until(Duration::from_secs(2), || title_of(dialog)
        == "PathWarp focus lock: locked"));
    press_ctrl_g();
    assert!(
        wait_until(Duration::from_secs(3), || automation
            .create_matcher()
            .from_ref(&root)
            .name("窗口未取得焦点，请点击目标窗口。")
            .timeout(0)
            .find_first()
            .is_ok()),
        "denied activation was not reported"
    );
    assert!(
        is_foreground(dialog),
        "the fixture's foreground lock must be respected"
    );
    host.send("unlock");
    assert!(wait_until(Duration::from_secs(2), || title_of(dialog)
        == "PathWarp focus lock: unlocked"));
    assert_shortcut_focus(&automation, app_pid, overlay);
    assert!(
        wait_until(Duration::from_secs(2), || automation
            .create_matcher()
            .from_ref(&root)
            .name("窗口未取得焦点，请点击目标窗口。")
            .timeout(0)
            .find_first()
            .is_err()),
        "a successful later activation kept the stale failure notice"
    );
}
