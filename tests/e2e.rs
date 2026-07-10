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

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput,
    SetFocus,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SetCursorPos, SetForegroundWindow,
};
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

/// 回归（核心）：点击悬浮条绝不能激活悬浮窗自身——这正是用户报告“点击即消失”的
/// 根因（旧实现里点击激活悬浮窗 → 对话框失去前台 → 悬浮条被隐藏）。
///
/// 断言口径为“悬浮窗自身永不成为前台”，对环境里其它抢焦点的工具（如同时运行的
/// Listary，会在文件对话框获焦时弹出自己的搜索条）鲁棒——那类干扰会抢走对话框的
/// 前台，但绝不会把前台交给*我们的*悬浮窗；只有本 bug 复发时悬浮窗才会自我激活。
///
/// 停靠持续性（点击后仍紧贴对话框）依赖干净桌面，见 `overlay_recovers_after_click_and_reopen`
/// 与 `window_ext` 的确定性子类化单测。
#[test]
#[ignore = "requires interactive desktop; run via `just e2e`"]
fn clicking_overlay_never_activates_it() {
    let (_app, app_pid) = spawn_pathwarp();
    let mut host = DialogHost::spawn();

    host.open_dialog();
    let (overlay_hwnd, overlay_rect) =
        wait_for(Duration::from_secs(5), || overlay_onscreen(app_pid))
            .expect("overlay did not dock after dialog opened");

    let center = center_of(&overlay_rect);
    click_at(center.x, center.y);

    // 点击后连续采样一段时间：悬浮窗自身在任何时刻都不得成为前台窗口。
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(50));
        let fg = unsafe { GetForegroundWindow().0 as isize };
        assert_ne!(
            fg,
            overlay_hwnd,
            "overlay window activated itself on click (WS_EX_NOACTIVATE / MA_NOACTIVATE regression); fg_class='{}'",
            class_of(fg),
        );
    }
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
