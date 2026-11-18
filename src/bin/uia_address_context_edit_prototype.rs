//! THROWAWAY: one corrected context-menu edit-entry case.
//! Run: cargo run --bin uia_address_context_edit_prototype
//! At most one context request and one strictly guarded Edit-address Invoke.
//! Derived from the accepted read-only inventory, retained unchanged beside this file.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{E_ABORT, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
    IUIAutomationElement3, IUIAutomationInvokePattern, IUIAutomationTreeWalker,
    IUIAutomationValuePattern, TreeScope_Descendants, UIA_ControlTypePropertyId,
    UIA_InvokePatternId, UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FOS_DONTADDTORECENT, FOS_FORCEFILESYSTEM, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileDialogEvents, IFileDialogEvents_Impl, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GetClassNameW, GetForegroundWindow, GetParent,
    GetWindow, GetWindowThreadProcessId, IsChild, IsWindow, IsWindowVisible, KillTimer,
    PostMessageW, SetTimer, WM_CLOSE,
};
use windows::core::{BOOL, HSTRING, Interface, PWSTR, Ref, implement, w};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_millis())
}

thread_local! {
    static FIXTURE: RefCell<Option<(IFileDialog, PathBuf)>> = const { RefCell::new(None) };
}

unsafe extern "system" fn observe_tick(_: HWND, _: u32, _: usize, _: u32) {
    FIXTURE.with(|slot| {
        if let Some((dialog, root)) = slot.borrow().as_ref()
            && let Ok(mut state) = snapshot(dialog, root)
        {
            state["event"] = json!("host_poll");
            println!("{state}");
        }
    });
}

fn main() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--host") {
        return host(&args[2], Path::new(&args[3]));
    }
    if args.get(1).is_some_and(|arg| arg == "--client") {
        let window = HWND(args[2].parse::<usize>()? as *mut _);
        return probe(window, Path::new(&args[3]), args[4].parse()?);
    }
    if args.len() != 1 {
        return Err("usage: uia_address_context_edit_prototype".into());
    }
    run()
}

fn text(value: PWSTR) -> windows::core::Result<String> {
    let result = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(result?)
}

fn folder(dialog: &IFileDialog) -> windows::core::Result<String> {
    text(unsafe { dialog.GetFolder()?.GetDisplayName(SIGDN_FILESYSPATH)? })
}

fn snapshot(dialog: &IFileDialog, root: &Path) -> windows::core::Result<Value> {
    let window = unsafe { dialog.cast::<IOleWindow>()?.GetWindow()? };
    Ok(json!({
        "event": "folder_snapshot",
        "hwnd": window.0 as usize,
        "folder": folder(dialog)?.replace(root.to_string_lossy().as_ref(), "PROTOTYPE_ROOT"),
        "filename": text(unsafe { dialog.GetFileName()? })?,
        "file_type": unsafe { dialog.GetFileTypeIndex()? },
        "dialog_visible": unsafe { IsWindowVisible(window).as_bool() },
        "dialog_alive": unsafe { IsWindow(Some(window)).as_bool() },
        "observed_at_ms": now_ms(),
    }))
}

#[implement(IFileDialogEvents)]
struct FolderObserver {
    root: PathBuf,
}

impl IFileDialogEvents_Impl for FolderObserver_Impl {
    fn OnFileOk(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
        Err(E_ABORT.into()) // This fixture never accepts an Open/Save operation.
    }
    fn OnFolderChanging(
        &self,
        _: Ref<IFileDialog>,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChange(&self, dialog: Ref<IFileDialog>) -> windows::core::Result<()> {
        if let Ok(state) = snapshot(dialog.ok()?, &self.root) {
            println!("{state}");
        }
        Ok(())
    }
    fn OnSelectionChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnShareViolation(
        &self,
        _: Ref<IFileDialog>,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> {
        Ok(FDESVR_DEFAULT)
    }
    fn OnTypeChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnOverwrite(
        &self,
        _: Ref<IFileDialog>,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> {
        Ok(FDEOR_DEFAULT)
    }
}

fn host(mode: &str, root: &Path) -> AnyResult<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let class = if mode == "save" {
        &FileSaveDialog
    } else {
        &FileOpenDialog
    };
    let dialog: IFileDialog = unsafe { CoCreateInstance(class, None, CLSCTX_INPROC_SERVER)? };
    let initial: IShellItem = unsafe {
        SHCreateItemFromParsingName(
            &HSTRING::from(root.join("A").to_string_lossy().as_ref()),
            None,
        )?
    };
    unsafe {
        dialog.SetOptions(dialog.GetOptions()? | FOS_DONTADDTORECENT | FOS_FORCEFILESYSTEM)?;
        dialog.SetTitle(&HSTRING::from(format!(
            "PROTOTYPE — context-menu address edit entry ({mode})"
        )))?;
        dialog.SetFileTypes(&[
            COMDLG_FILTERSPEC {
                pszName: w!("Text"),
                pszSpec: w!("*.txt"),
            },
            COMDLG_FILTERSPEC {
                pszName: w!("CSV"),
                pszSpec: w!("*.csv"),
            },
        ])?;
        dialog.SetFileTypeIndex(2)?;
        dialog.SetFileName(w!("pending-prototype-name.csv"))?;
        dialog.SetFolder(&initial)?;
    }
    let observer: IFileDialogEvents = FolderObserver {
        root: root.to_owned(),
    }
    .into();
    let cookie = unsafe { dialog.Advise(&observer)? };
    FIXTURE.with(|slot| *slot.borrow_mut() = Some((dialog.clone(), root.to_owned())));
    let timer = unsafe { SetTimer(None, 0, 100, Some(observe_tick)) };
    if timer == 0 {
        return Err("fixture observer timer could not be created".into());
    }
    std::thread::spawn(|| {
        for line in std::io::stdin().lock().lines() {
            if line.is_ok_and(|line| line == "close") {
                close_fixture();
                break;
            }
        }
    });
    let result = unsafe { dialog.Show(None) };
    unsafe { KillTimer(None, timer)? };
    FIXTURE.with(|slot| *slot.borrow_mut() = None);
    println!(
        "{}",
        json!({"event": "host_closed", "result": format!("{result:?}")})
    );
    unsafe {
        dialog.Unadvise(cookie)?;
    }
    Ok(())
}

fn close_fixture() {
    unsafe extern "system" fn visit(window: HWND, _: LPARAM) -> BOOL {
        let mut pid = 0;
        let mut class = [0_u16; 64];
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
            let length = GetClassNameW(window, &mut class);
            if pid == GetCurrentProcessId()
                && usize::try_from(length)
                    .is_ok_and(|n| String::from_utf16_lossy(&class[..n]) == "#32770")
            {
                let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        BOOL(1)
    }
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(0)) };
}

fn run() -> AnyResult<()> {
    let mode = "open";
    let root = std::env::temp_dir().join(format!(
        "PathWarp-CONTEXTEDIT-PROTOTYPE-{}-{mode}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("A").join("marker-only-in-A"))?;
    std::fs::create_dir_all(root.join("B").join("marker-only-in-B"))?;
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--host", mode])
        .arg(&root)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let (send, receive) = mpsc::channel();
    let output = child.stdout.take().ok_or("missing fixture stdout")?;
    let host_reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(output)
            .lines()
            .map_while(Result::ok)
        {
            println!("{line}");
            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                let _ = send.send(event);
            }
        }
    });
    let result = (|| -> AnyResult<()> {
        println!(
            "{}",
            json!({"event": "probe_start", "mode": mode, "architecture": std::env::consts::ARCH, "host_pid": child.id(), "supervisor_pid": std::process::id()})
        );
        let ready = receive.recv_timeout(Duration::from_secs(15))?;
        let raw = usize::try_from(
            ready["hwnd"]
                .as_u64()
                .ok_or("fixture did not report its HWND")?,
        )?;
        let window = HWND(raw as *mut _);
        let visible_deadline = Instant::now() + Duration::from_secs(10);
        while !unsafe { IsWindowVisible(window).as_bool() } {
            if Instant::now() >= visible_deadline || child.try_wait()?.is_some() {
                return Err("fixture did not become visible".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let before = loop {
            let event = receive.recv_timeout(Duration::from_millis(500))?;
            if event["event"] == "host_poll"
                && event["dialog_visible"] == true
                && event["filename"] == "pending-prototype-name.csv"
                && event["file_type"] == 2
                && event["folder"] == "PROTOTYPE_ROOT\\A"
            {
                break event;
            }
            if Instant::now() >= deadline {
                return Err(
                    "fixture did not expose a fully initialized filename/type baseline".into(),
                );
            }
        };
        println!("{}", json!({"event": "baseline", "state": before}));
        // A separate client process bounds every provider call, including read-only queries.
        // The supervisor retains ownership of the fixture even if UIA blocks.
        let mut client = Command::new(std::env::current_exe()?)
            .arg("--client")
            .arg(raw.to_string())
            .arg(&root)
            .arg(child.id().to_string())
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdout(Stdio::piped())
            .spawn()?;
        let output = client.stdout.take().ok_or("missing UIA client stdout")?;
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(output)
                .lines()
                .map_while(Result::ok)
            {
                println!("{line}");
            }
        });
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = client.try_wait()? {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(15) {
                timed_out = true;
                client.kill()?; // This command's UIA worker only; never retry the pass.
                break client.wait()?;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let _ = reader.join();
        println!(
            "{}",
            json!({
                "event": "client_exit", "success": status.success(), "timed_out": timed_out,
                "elapsed_ms": started.elapsed().as_millis(), "total_timeout_ms": 15000,
                "note": "A timeout leaves the active query incomplete; there is no retry.",
            })
        );
        let after = observe(
            &receive,
            &before,
            "after_client",
            Duration::from_millis(350),
        );
        println!(
            "{}",
            json!({
                "event": "native_verdict", "before": before, "after": after,
                "folder_unchanged": after["folder"] == before["folder"],
                "filename_preserved": after["filename"] == before["filename"],
                "file_type_preserved": after["file_type"] == before["file_type"],
                "same_dialog_alive": unsafe { IsWindow(Some(window)).as_bool() },
                "successful_jump_tested": false,
            })
        );
        if !status.success() || timed_out {
            return Err("UIA worker failed or reached its bound; inspect the trace".into());
        }
        Ok(())
    })();
    if let Some(input) = child.stdin.as_mut() {
        let _ = writeln!(input, "close");
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill()?; // Only this command's own fixture process.
            child.wait()?;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let _ = host_reader.join();
    println!(
        "{}",
        json!({"event": "fixture_cleanup", "host_exited": true})
    );
    result
}

fn scrub(value: &str, root: &Path) -> String {
    let mut value = value.replace(root.to_string_lossy().as_ref(), "PROTOTYPE_ROOT");
    if let Some(name) = root.file_name().and_then(|part| part.to_str()) {
        value = value.replace(name, "PROTOTYPE_ROOT");
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        value = value.replace(&profile, "USER_PROFILE");
    }
    if std::env::var("USERNAME").is_ok_and(|name| value == name) {
        value = "USER_NAME".to_owned();
    }
    if value.contains(":\\") || value.starts_with("\\\\") || value.contains("USER_PROFILE") {
        value = "NON_FIXTURE_PATH".to_owned();
    }
    value
}

fn observe(
    events: &mpsc::Receiver<Value>,
    before: &Value,
    phase: &str,
    duration: Duration,
) -> Value {
    let mut last = before.clone();
    let mut polls = 0;
    let deadline = Instant::now() + duration;
    let started_at = now_ms();
    while Instant::now() < deadline {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(100))
            && event["event"] == "host_poll"
            && event["observed_at_ms"]
                .as_u64()
                .is_some_and(|value| u128::from(value) >= started_at)
        {
            last = event;
            polls += 1;
        }
    }
    println!(
        "{}",
        json!({"event": "native_observation", "phase": phase, "started_at_ms": started_at, "fresh_polls": polls, "state": last})
    );
    last
}

fn native_info(window: HWND) -> Value {
    let mut pid = 0;
    let mut class = [0_u16; 256];
    let length = unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        GetClassNameW(window, &mut class)
    };
    json!({
        "hwnd": window.0 as usize, "pid": pid,
        "class": usize::try_from(length).ok().map(|n| String::from_utf16_lossy(&class[..n])),
        "visible": unsafe { IsWindowVisible(window).as_bool() },
        "alive": unsafe { IsWindow(Some(window)).as_bool() },
        "parent": unsafe { GetParent(window) }.ok().map(|parent| parent.0 as usize),
        "owner": unsafe { GetWindow(window, GW_OWNER) }.ok().map(|owner| owner.0 as usize),
    })
}

fn emit(value: Value) {
    println!("{value}");
}

struct Shared {
    started: Instant,
    calls: AtomicU32,
    stop: Mutex<Option<String>>,
    show_started: AtomicBool,
    edit_started: AtomicBool,
}

#[derive(Clone)]
struct Budget {
    shared: Arc<Shared>,
    dialog: usize,
    pid: u32,
}

impl Budget {
    fn new(dialog: HWND, pid: u32) -> Self {
        Self {
            shared: Arc::new(Shared {
                started: Instant::now(),
                calls: AtomicU32::new(0),
                stop: Mutex::new(None),
                show_started: AtomicBool::new(false),
                edit_started: AtomicBool::new(false),
            }),
            dialog: dialog.0 as usize,
            pid,
        }
    }

    fn reason(&self) -> Option<String> {
        self.shared
            .stop
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn stop(&self, reason: &str) {
        let mut slot = self
            .shared
            .stop
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.is_none() {
            *slot = Some(reason.to_owned());
            emit(json!({"event":"pass_stop","reason":reason,"observed_at_ms":now_ms()}));
        }
    }

    fn open(&self) -> bool {
        if self.reason().is_some() {
            return false;
        }
        let window = HWND(self.dialog as *mut _);
        if !native_owned(window, self.pid) || !unsafe { IsWindow(Some(window)).as_bool() } {
            self.stop("original_dialog_identity_lost");
        } else if self.shared.started.elapsed() >= Duration::from_secs(10) {
            self.stop("soft_time_budget");
        } else if self.shared.calls.load(Ordering::SeqCst) >= 512 {
            self.stop("call_budget");
        }
        self.reason().is_none()
    }

    fn call<T>(
        &self,
        kind: &str,
        label: &str,
        operation: impl FnOnce() -> windows::core::Result<T>,
    ) -> Option<windows::core::Result<T>> {
        if !self.open() {
            return None;
        }
        let Ok(previous) =
            self.shared
                .calls
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    (value < 512).then_some(value + 1)
                })
        else {
            self.stop("call_budget");
            return None;
        };
        let call = previous + 1;
        emit(
            json!({"event":"call_start","call":call,"kind":kind,"label":label,"observed_at_ms":now_ms()}),
        );
        let _ = std::io::stdout().flush();
        let started = Instant::now();
        let result = operation();
        emit(
            json!({"event":"call_end","call":call,"kind":kind,"label":label,
            "rust_result":if result.is_ok(){"ok"}else{"err"},
            "rust_error_code":result.as_ref().err().map(|error|format!("{:?}",error.code())),
            "raw_hresult":null,"elapsed_us":started.elapsed().as_micros(),"observed_at_ms":now_ms()}),
        );
        Some(result)
    }

    fn read<T>(
        &self,
        label: &str,
        operation: impl FnOnce() -> windows::core::Result<T>,
    ) -> Option<windows::core::Result<T>> {
        self.call("read", label, operation)
    }
}

fn need<T>(result: Option<windows::core::Result<T>>) -> AnyResult<T> {
    Ok(result.ok_or("budget stopped before query")??)
}

fn outcome<T>(
    result: &Option<windows::core::Result<T>>,
    render: impl FnOnce(&T) -> Value,
) -> Value {
    match result {
        None => json!({"status":"not_requested"}),
        Some(Ok(value)) => json!({"status":"ok","value":render(value)}),
        Some(Err(error)) if error.code().is_ok() => {
            json!({"status":"binding_null_unknown","rust_error_code":format!("{:?}",error.code()),"raw_hresult":null})
        }
        Some(Err(error)) => {
            json!({"status":"error","rust_error_code":format!("{:?}",error.code())})
        }
    }
}

fn native_owned(window: HWND, pid: u32) -> bool {
    let mut actual = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut actual)) };
    actual == pid && unsafe { IsWindow(Some(window)).as_bool() }
}

fn class_name(window: HWND) -> String {
    let mut buffer = [0_u16; 256];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    usize::try_from(length).map_or_else(
        |_| String::new(),
        |length| String::from_utf16_lossy(&buffer[..length]),
    )
}

fn descendants(window: HWND) -> Vec<HWND> {
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
        unsafe { &mut *(data.0 as *mut Vec<HWND>) }.push(window);
        BOOL(1)
    }
    let mut result = Vec::new();
    let _ = unsafe {
        EnumChildWindows(
            Some(window),
            Some(collect),
            LPARAM((&raw mut result) as isize),
        )
    };
    result
}

fn own_top_windows(pid: u32) -> Vec<HWND> {
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
        let (pid, result) = unsafe { &mut *(data.0 as *mut (u32, Vec<HWND>)) };
        if native_owned(window, *pid) {
            result.push(window);
        }
        BOOL(1)
    }
    let mut data = (pid, Vec::new());
    let _ = unsafe { EnumWindows(Some(collect), LPARAM((&raw mut data) as isize)) };
    data.1
}

fn normalize_caption(value: &str) -> String {
    let text = value
        .split('\t')
        .next()
        .unwrap_or_default()
        .replace('&', "");
    let trimmed = text.trim();
    let base = if trimmed.ends_with(')') {
        trimmed
            .rfind('(')
            .and_then(|index| {
                let key = &trimmed[index + 1..trimmed.len() - 1];
                (key.len() == 1 && key.bytes().all(|value| value.is_ascii_alphabetic()))
                    .then_some(&trimmed[..index])
            })
            .unwrap_or(trimmed)
    } else {
        trimmed
    };
    base.trim().to_owned()
}

fn is_edit_caption(value: &str) -> bool {
    let normalized = normalize_caption(value);
    normalized == "编辑地址" || normalized.eq_ignore_ascii_case("Edit address")
}

fn metadata(budget: &Budget, element: &IUIAutomationElement, label: &str, root: &Path) -> Value {
    macro_rules! attr {
        ($name:ident, $render:expr) => {
            outcome(
                &budget.read(&format!("{label}.{}", stringify!($name)), || unsafe {
                    element.$name()
                }),
                $render,
            )
        };
    }
    json!({
        "pid":attr!(CurrentProcessId,|v|json!(v)),
        "name":attr!(CurrentName,|v|json!(scrub(&v.to_string(),root))),
        "class":attr!(CurrentClassName,|v|json!(v.to_string())),
        "automation_id":attr!(CurrentAutomationId,|v|json!(v.to_string())),
        "control_type":attr!(CurrentControlType,|v|json!(v.0)),
        "enabled":attr!(CurrentIsEnabled,|v|json!(v.as_bool())),
        "offscreen":attr!(CurrentIsOffscreen,|v|json!(v.as_bool())),
        "native_hwnd":attr!(CurrentNativeWindowHandle,|v|json!(v.0 as usize)),
    })
}

fn meta_pid(meta: &Value, pid: u32) -> bool {
    meta["pid"]["status"] == "ok" && meta["pid"]["value"].as_i64() == Some(i64::from(pid))
}

fn toolbar_native_guard(dialog: HWND, toolbar: HWND, pid: u32) -> bool {
    let Ok(parent) = (unsafe { GetParent(toolbar) }) else {
        return false;
    };
    native_owned(dialog, pid)
        && native_owned(toolbar, pid)
        && native_owned(parent, pid)
        && unsafe { IsChild(dialog, toolbar).as_bool() }
        && class_name(toolbar) == "ToolbarWindow32"
        && class_name(parent) == "Breadcrumb Parent"
        && unsafe { IsWindowVisible(toolbar).as_bool() && IsWindowEnabled(toolbar).as_bool() }
}

fn fresh_toolbar(
    budget: &Budget,
    uia: &IUIAutomation,
    dialog: HWND,
    toolbar: HWND,
) -> AnyResult<IUIAutomationElement> {
    if !toolbar_native_guard(dialog, toolbar, budget.pid) {
        return Err("toolbar native identity/ancestry/visibility gate failed".into());
    }
    let element = need(budget.read("toolbar.ElementFromHandle", || unsafe {
        uia.ElementFromHandle(toolbar)
    }))?;
    let actual = need(budget.read("toolbar.CurrentNativeWindowHandle", || unsafe {
        element.CurrentNativeWindowHandle()
    }))?;
    let pid = need(budget.read("toolbar.CurrentProcessId", || unsafe {
        element.CurrentProcessId()
    }))?;
    let class = need(budget.read("toolbar.CurrentClassName", || unsafe {
        element.CurrentClassName()
    }))?;
    let automation_id = need(budget.read("toolbar.CurrentAutomationId", || unsafe {
        element.CurrentAutomationId()
    }))?;
    let enabled = need(budget.read("toolbar.CurrentIsEnabled", || unsafe {
        element.CurrentIsEnabled()
    }))?;
    let offscreen = need(budget.read("toolbar.CurrentIsOffscreen", || unsafe {
        element.CurrentIsOffscreen()
    }))?;
    let rebound = need(budget.read("toolbar.identity_rebind", || unsafe {
        uia.ElementFromHandle(toolbar)
    }))?;
    let same = need(budget.read("toolbar.CompareElements", || unsafe {
        uia.CompareElements(&element, &rebound)
    }))?;
    if actual != toolbar
        || i64::from(pid) != i64::from(budget.pid)
        || class != "ToolbarWindow32"
        || automation_id != "1001"
        || !enabled.as_bool()
        || offscreen.as_bool()
        || !same.as_bool()
    {
        return Err("fresh toolbar UIA identity/state gate failed".into());
    }
    Ok(element)
}

fn start_context_request(budget: Budget, toolbar: usize) -> mpsc::Receiver<bool> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> AnyResult<()> {
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
            let uia: IUIAutomation =
                unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
            let element = fresh_toolbar(
                &budget,
                &uia,
                HWND(budget.dialog as *mut _),
                HWND(toolbar as *mut _),
            )?;
            let version = need(budget.read("toolbar.QueryInterface.Element3", || {
                element.cast::<IUIAutomationElement3>()
            }))?;
            need(budget.call("action", "toolbar.ShowContextMenu", || {
                budget.shared.show_started.store(true, Ordering::SeqCst);
                unsafe { version.ShowContextMenu() }
            }))?;
            Ok(())
        })();
        let success = result.is_ok();
        if !success {
            budget.stop("context_request_or_preflight_failed");
        }
        emit(
            json!({"event":"context_request_finished","success":success,"error":result.err().map(|e|e.to_string()),"raw_hresult":null,"observed_at_ms":now_ms()}),
        );
        let _ = send.send(success);
    });
    receive
}

struct Candidate {
    element: IUIAutomationElement,
    menu: IUIAutomationElement,
    native_seed: Option<usize>,
    index: usize,
}

struct Inspector {
    budget: Budget,
    uia: IUIAutomation,
    walker: IUIAutomationTreeWalker,
    menu_condition: IUIAutomationCondition,
    dialog: IUIAutomationElement,
    root: PathBuf,
    seen: Vec<IUIAutomationElement>,
    records: Vec<Value>,
    seeds: HashSet<usize>,
    window_cache: HashMap<usize, IUIAutomationElement>,
    sampled_menus: HashMap<usize, Instant>,
}

impl Inspector {
    fn remember(&mut self, element: &IUIAutomationElement, source: &str) -> AnyResult<usize> {
        for (index, prior) in self.seen.iter().enumerate() {
            if need(self.budget.read("identity.CompareElements", || unsafe {
                self.uia.CompareElements(prior, element)
            }))?
            .as_bool()
            {
                return Ok(index);
            }
        }
        if self.seen.len() >= 64 {
            self.budget.stop("element_budget");
            return Err("element budget reached".into());
        }
        let pid = need(self.budget.read("element.scope_pid", || unsafe {
            element.CurrentProcessId()
        }))?;
        if i64::from(pid) != i64::from(self.budget.pid) {
            self.budget.stop("element_process_identity_mismatch");
            return Err("element PID mismatch; metadata not captured".into());
        }
        let index = self.seen.len();
        let data = metadata(&self.budget, element, &format!("node_{index}"), &self.root);
        self.seen.push(element.clone());
        self.records.push(data.clone());
        emit(
            json!({"event":"element","index":index,"source":source,"data":data,"observed_at_ms":now_ms()}),
        );
        Ok(index)
    }

    fn seed(&mut self, window: HWND) -> AnyResult<IUIAutomationElement> {
        if !native_owned(window, self.budget.pid) {
            return Err("native seed identity lost".into());
        }
        let raw = window.0 as usize;
        if let Some(element) = self.window_cache.get(&raw) {
            return Ok(element.clone());
        }
        if self.seeds.len() >= 16 {
            self.budget.stop("window_seed_budget");
            return Err("window seed budget reached".into());
        }
        self.seeds.insert(raw);
        let element = need(self.budget.read("seed.ElementFromHandle", || unsafe {
            self.uia.ElementFromHandle(window)
        }))?;
        let actual = need(
            self.budget
                .read("seed.CurrentNativeWindowHandle", || unsafe {
                    element.CurrentNativeWindowHandle()
                }),
        )?;
        if actual != window {
            self.budget.stop("seed_native_binding_mismatch");
            return Err("seed HWND mismatch".into());
        }
        self.remember(&element, "native_seed")?;
        self.window_cache.insert(raw, element.clone());
        Ok(element)
    }

    fn menus_below(&self, element: &IUIAutomationElement) -> AnyResult<Vec<IUIAutomationElement>> {
        let array = need(self.budget.read("FindAll.MenuOnly", || unsafe {
            element.FindAll(TreeScope_Descendants, &self.menu_condition)
        }))?;
        let length = need(
            self.budget
                .read("MenuArray.Length", || unsafe { array.Length() }),
        )?;
        if !(0..=64).contains(&length) {
            self.budget.stop("menu_array_budget");
            return Err("menu array outside bounds".into());
        }
        (0..length)
            .map(|index| {
                need(self.budget.read("MenuArray.GetElement", || unsafe {
                    array.GetElement(index)
                }))
            })
            .collect()
    }

    fn child(
        &self,
        label: &str,
        operation: impl FnOnce() -> windows::core::Result<IUIAutomationElement>,
    ) -> AnyResult<Option<IUIAutomationElement>> {
        match self.budget.read(label, operation) {
            Some(Ok(element)) => Ok(Some(element)),
            Some(Err(error)) if error.code().is_ok() => {
                emit(json!({"event":"tree_end_binding_null","label":label,"raw_hresult":null}));
                Ok(None)
            }
            Some(Err(error)) => {
                self.budget.stop("menu_tree_read_error");
                Err(error.into())
            }
            None => Err("budget stopped during menu tree read".into()),
        }
    }

    fn walk_menu(
        &mut self,
        menu: IUIAutomationElement,
        native_seed: Option<usize>,
        candidates: &mut Vec<Candidate>,
        force_fresh: bool,
    ) -> AnyResult<()> {
        let menu_index = self.remember(&menu, "menu_root")?;
        if !force_fresh
            && self
                .sampled_menus
                .get(&menu_index)
                .is_some_and(|at| at.elapsed() < Duration::from_millis(900))
        {
            return Ok(());
        }
        self.sampled_menus.insert(menu_index, Instant::now());
        let mut pending = vec![(menu.clone(), None::<usize>, 0_usize)];
        while let Some((element, parent, depth)) = pending.pop() {
            if !self.budget.open() {
                return Err("budget stopped during menu capture".into());
            }
            if depth > 8 {
                self.budget.stop("depth_budget");
                return Err("menu depth budget reached".into());
            }
            let index = self.remember(&element, "menu_branch")?;
            if force_fresh {
                self.records[index] = metadata(
                    &self.budget,
                    &element,
                    &format!("node_{index}.confirm"),
                    &self.root,
                );
            }
            let data = &self.records[index];
            emit(
                json!({"event":"menu_edge","index":index,"parent":parent,"menu_root":menu_index,"native_seed":native_seed,"depth":depth,"observed_at_ms":now_ms()}),
            );
            if data["control_type"]["value"] == UIA_MenuItemControlTypeId.0
                && data["name"]["value"].as_str().is_some_and(is_edit_caption)
                && data["enabled"]["value"] == true
                && data["offscreen"]["value"] == false
                && !candidates.iter().any(|candidate| candidate.index == index)
            {
                emit(
                    json!({"event":"edit_caption_candidate","index":index,"raw_caption":data["name"],"normalized":data["name"]["value"].as_str().map(normalize_caption),"observed_at_ms":now_ms()}),
                );
                candidates.push(Candidate {
                    element: element.clone(),
                    menu: menu.clone(),
                    native_seed,
                    index,
                });
            }
            let mut children = Vec::new();
            let mut current = self.child("MenuWalker.GetFirstChild", || unsafe {
                self.walker.GetFirstChildElement(&element)
            })?;
            while let Some(child) = current {
                if children.len() >= 64 {
                    self.budget.stop("pending_children_budget");
                    return Err("menu child budget".into());
                }
                current = self.child("MenuWalker.GetNextSibling", || unsafe {
                    self.walker.GetNextSiblingElement(&child)
                })?;
                children.push(child);
            }
            for child in children.into_iter().rev() {
                pending.push((child, Some(index), depth + 1));
            }
        }
        Ok(())
    }

    fn discover(
        &mut self,
        before: &HashSet<usize>,
        force_fresh: bool,
    ) -> AnyResult<Vec<Candidate>> {
        // Retain independent native popup evidence before any possibly blocking UIA lookup.
        let own_windows = own_top_windows(self.budget.pid);
        let visible_popups: Vec<_> = own_windows
            .iter()
            .copied()
            .filter(|window| {
                let raw = window.0 as usize;
                raw != self.budget.dialog
                    && unsafe { IsWindowVisible(*window).as_bool() }
                    && (!before.contains(&raw) || class_name(*window) == "#32768")
            })
            .map(native_info)
            .collect();
        emit(
            json!({"event":"native_popup_poll","windows":visible_popups,"force_fresh":force_fresh,"observed_at_ms":now_ms()}),
        );
        let mut roots: Vec<(IUIAutomationElement, Option<usize>)> = self
            .menus_below(&self.dialog)?
            .into_iter()
            .map(|menu| (menu, None))
            .collect();
        for window in own_windows {
            let raw = window.0 as usize;
            if raw == self.budget.dialog || !unsafe { IsWindowVisible(window).as_bool() } {
                continue;
            }
            if before.contains(&raw) && class_name(window) != "#32768" {
                continue;
            }
            if !self.window_cache.contains_key(&raw) {
                emit(
                    json!({"event":"owned_popup_discovery","state":native_info(window),"present_before":before.contains(&raw),"observed_at_ms":now_ms()}),
                );
            }
            if class_name(window) == "SysShadow" {
                continue;
            }
            let element = self.seed(window)?;
            let control = need(self.budget.read("popup.CurrentControlType", || unsafe {
                element.CurrentControlType()
            }))?;
            if control == UIA_MenuControlTypeId {
                roots.push((element, Some(raw)));
            } else {
                roots.extend(
                    self.menus_below(&element)?
                        .into_iter()
                        .map(|menu| (menu, Some(raw))),
                );
            }
        }
        let mut candidates = Vec::new();
        let mut roots_seen = HashSet::new();
        for (menu, seed) in roots {
            let index = self.remember(&menu, "menu_root_identity")?;
            if roots_seen.insert(index) {
                self.walk_menu(menu, seed, &mut candidates, force_fresh)?;
            }
        }
        Ok(candidates)
    }

    fn invoke_edit(&self, candidate: &Candidate) -> AnyResult<()> {
        if !self.budget.open() {
            return Err("budget stopped before command guard".into());
        }
        if candidate.native_seed.is_some_and(|raw| {
            let window = HWND(raw as *mut _);
            !native_owned(window, self.budget.pid) || !unsafe { IsWindowVisible(window).as_bool() }
        }) {
            return Err("popup native identity/visibility lost".into());
        }
        let fresh = metadata(
            &self.budget,
            &candidate.element,
            "edit_command.fresh",
            &self.root,
        );
        if !meta_pid(&fresh, self.budget.pid)
            || fresh["control_type"]["value"] != UIA_MenuItemControlTypeId.0
            || !fresh["name"]["value"].as_str().is_some_and(is_edit_caption)
            || fresh["enabled"]["value"] != true
            || fresh["offscreen"]["value"] != false
        {
            return Err("fresh Edit-address semantic/state guard failed".into());
        }
        let mut ancestor = candidate.element.clone();
        let mut reached_menu = false;
        for _ in 0..=8 {
            if need(
                self.budget
                    .read("edit_command.CompareMenuAncestor", || unsafe {
                        self.uia.CompareElements(&ancestor, &candidate.menu)
                    }),
            )?
            .as_bool()
            {
                reached_menu = true;
                break;
            }
            ancestor = need(self.budget.read("edit_command.GetParent", || unsafe {
                self.walker.GetParentElement(&ancestor)
            }))?;
        }
        if !reached_menu {
            return Err("fresh menu ancestor identity not established".into());
        }
        let pattern_result =
            self.budget
                .read("edit_command.GetCurrentPatternAs.Invoke", || unsafe {
                    candidate
                        .element
                        .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                });
        emit(
            json!({"event":"edit_invoke_binding","index":candidate.index,"binding":outcome(&pattern_result,|_|json!("interface_obtained")),"fresh":fresh,"observed_at_ms":now_ms()}),
        );
        let pattern = need(pattern_result)?;
        need(self.budget.call("action", "edit_command.Invoke", || {
            if self.budget.shared.edit_started.swap(true, Ordering::SeqCst) {
                return Err(E_ABORT.into());
            }
            unsafe { pattern.Invoke() }
        }))?;
        Ok(())
    }

    fn address_state(&mut self, band: HWND, phase: &str, read_value: bool) -> AnyResult<bool> {
        let windows = descendants(band);
        let mut writable = false;
        for class in ["ComboBoxEx32", "ComboBox", "Edit"] {
            let matches: Vec<_> = windows
                .iter()
                .copied()
                .filter(|window| {
                    native_owned(*window, self.budget.pid) && class_name(*window) == class
                })
                .collect();
            if matches.len() != 1 {
                emit(
                    json!({"event":"address_state_unknown","class":class,"count":matches.len(),"phase":phase}),
                );
                continue;
            }
            let window = matches[0];
            let state = native_info(window);
            emit(
                json!({"event":"native_address_state","class":class,"phase":phase,"state":state,"observed_at_ms":now_ms()}),
            );
            if class == "Edit" && state["visible"] == true && read_value {
                let element = self.seed(window)?;
                let pattern =
                    self.budget
                        .read("visible_edit.GetCurrentPatternAs.Value", || unsafe {
                            element.GetCurrentPatternAs::<IUIAutomationValuePattern>(
                                UIA_ValuePatternId,
                            )
                        });
                let status = outcome(&pattern, |_| json!("interface_obtained"));
                if let Some(Ok(pattern)) = pattern {
                    let readonly = self
                        .budget
                        .read("visible_edit.CurrentIsReadOnly", || unsafe {
                            pattern.CurrentIsReadOnly()
                        });
                    let value = self.budget.read("visible_edit.CurrentValue", || unsafe {
                        pattern.CurrentValue()
                    });
                    writable = matches!(readonly,Some(Ok(flag)) if !flag.as_bool());
                    emit(
                        json!({"event":"visible_editor_value","phase":phase,"hwnd":window.0 as usize,"binding":status,
                        "readonly":outcome(&readonly,|v|json!(v.as_bool())),"value":outcome(&value,|v|json!(scrub(&v.to_string(),&self.root))),
                        "visible_and_writable":writable,"observed_at_ms":now_ms()}),
                    );
                } else {
                    emit(
                        json!({"event":"visible_editor_value","phase":phase,"binding":status,"visible_and_writable":false,"observed_at_ms":now_ms()}),
                    );
                }
            }
        }
        Ok(writable)
    }

    fn focus(&self, phase: &str) {
        let focused = self.budget.read("focus.GetFocusedElement", || unsafe {
            self.uia.GetFocusedElement()
        });
        let pid = if let Some(Ok(element)) = focused {
            outcome(
                &self.budget.read("focus.CurrentProcessId", || unsafe {
                    element.CurrentProcessId()
                }),
                |pid| json!(pid),
            )
        } else {
            outcome(&focused, |_| Value::Null)
        };
        emit(
            json!({"event":"focus_foreground","phase":phase,"focused_pid":pid,"foreground":native_info(unsafe{GetForegroundWindow()}),"observed_at_ms":now_ms()}),
        );
    }
}

fn probe(window: HWND, root: &Path, host_pid: u32) -> AnyResult<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let budget = Budget::new(window, host_pid);
    emit(
        json!({"event":"case_start","host_pid":host_pid,"client_pid":std::process::id(),"observed_at_ms":now_ms(),
        "max_window_seeds":16,"max_unique_elements":64,"max_depth":8,"max_uia_calls":512,"soft_budget_ms":10000,"supervisor_ms":15000,
        "max_context_requests":1,"max_edit_invokes":1,"raw_success_hresult_retained":false}),
    );
    let uia: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let bands: Vec<_> = descendants(window)
        .into_iter()
        .filter(|hwnd| native_owned(*hwnd, host_pid) && class_name(*hwnd) == "Address Band Root")
        .collect();
    if bands.len() != 1 {
        return Err("address band is not unique".into());
    }
    let band = bands[0];
    let toolbars: Vec<_> = descendants(band)
        .into_iter()
        .filter(|hwnd| toolbar_native_guard(window, *hwnd, host_pid))
        .collect();
    if toolbars.len() != 1 {
        return Err("address breadcrumb toolbar is not unique".into());
    }
    let toolbar = toolbars[0];
    let target = fresh_toolbar(&budget, &uia, window, toolbar)?;
    let walker = need(budget.read("RawViewWalker", || unsafe { uia.RawViewWalker() }))?;
    let condition = need(budget.read("CreatePropertyCondition.Menu", || unsafe {
        uia.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_MenuControlTypeId.0),
        )
    }))?;
    let dialog = need(budget.read("dialog.ElementFromHandle", || unsafe {
        uia.ElementFromHandle(window)
    }))?;
    let mut inspector = Inspector {
        budget: budget.clone(),
        uia,
        walker,
        menu_condition: condition,
        dialog,
        root: root.to_owned(),
        seen: Vec::new(),
        records: Vec::new(),
        seeds: HashSet::from([window.0 as usize, toolbar.0 as usize]),
        window_cache: HashMap::new(),
        sampled_menus: HashMap::new(),
    };
    // Do not read the owned dialog's file-view metadata: it is a Menu-only query anchor.
    inspector
        .window_cache
        .insert(window.0 as usize, inspector.dialog.clone());
    inspector.remember(&target, "address_toolbar")?;
    inspector.address_state(band, "before", false)?;
    inspector.focus("before");
    let before_windows: HashSet<_> = own_top_windows(host_pid)
        .iter()
        .map(|window| window.0 as usize)
        .collect();
    emit(
        json!({"event":"owned_window_baseline","windows":own_top_windows(host_pid).iter().map(|window|native_info(*window)).collect::<Vec<_>>(),"observed_at_ms":now_ms()}),
    );
    let receive = start_context_request(budget.clone(), toolbar.0 as usize);
    let preparation_started = Instant::now();
    let mut discovery_started = None::<Instant>;
    let mut show_result = None;
    let mut invoked = false;
    let mut saw_writable = false;
    let mut discovery_complete = false;
    let mut post_observation_complete = false;
    let operation = (|| -> AnyResult<()> {
        loop {
            if let Ok(result) = receive.try_recv() {
                show_result = Some(result);
            }
            if !budget.open() {
                break;
            }
            if budget.shared.show_started.load(Ordering::SeqCst) && discovery_started.is_none() {
                discovery_started = Some(Instant::now());
                emit(json!({"event":"menu_discovery_begin","observed_at_ms":now_ms()}));
            }
            if let Some(started) = discovery_started {
                if started.elapsed() >= Duration::from_secs(3) {
                    discovery_complete = true;
                    break;
                }
                let candidates = inspector.discover(&before_windows, false)?;
                if candidates.len() > 1 {
                    budget.stop("edit_command_ambiguous");
                    break;
                }
                if let Some(candidate) = candidates.first() {
                    let confirmed = inspector.discover(&before_windows, true)?;
                    if confirmed.len() != 1 || confirmed[0].index != candidate.index {
                        budget.stop("fresh_command_uniqueness_or_identity_changed");
                        break;
                    }
                    inspector.invoke_edit(&confirmed[0])?;
                    invoked = true;
                    emit(
                        json!({"event":"edit_invoke_completed","index":candidate.index,"observed_at_ms":now_ms()}),
                    );
                    break;
                }
            } else if preparation_started.elapsed() >= Duration::from_secs(2) {
                budget.stop("context_preparation_not_completed");
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        emit(
            json!({"event":"menu_discovery_end","elapsed_ms":discovery_started.map(|start|start.elapsed().as_millis()),"invoked":invoked,"observation_complete":discovery_complete,"observed_at_ms":now_ms()}),
        );
        if invoked {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(3) && budget.open() {
                saw_writable |= inspector.address_state(band, "after_edit", true)?;
                std::thread::sleep(Duration::from_millis(500));
            }
            post_observation_complete =
                started.elapsed() >= Duration::from_secs(3) && budget.reason().is_none();
            emit(
                json!({"event":"editor_observation_end","elapsed_ms":started.elapsed().as_millis(),"visible_writable_observed":saw_writable,"complete":post_observation_complete,"observed_at_ms":now_ms()}),
            );
        }
        inspector.address_state(band, "final", true)?;
        if budget.open() {
            inspector.focus("after");
        }
        Ok(())
    })();
    if let Err(error) = operation {
        emit(
            json!({"event":"observation_error","error":scrub(&error.to_string(),root),"observed_at_ms":now_ms()}),
        );
        budget.stop("observation_or_command_guard_failed");
    }
    if let Ok(result) = receive.try_recv() {
        show_result = Some(result);
    }
    if show_result.is_none() {
        budget.stop("context_request_still_pending_or_incomplete");
    }
    emit(
        json!({"event":"case_done","calls":budget.shared.calls.load(Ordering::SeqCst),"unique_elements":inspector.seen.len(),"window_seeds":inspector.seeds.len(),
        "show_started":budget.shared.show_started.load(Ordering::SeqCst),"show_result":show_result,"edit_started":budget.shared.edit_started.load(Ordering::SeqCst),
        "edit_invoke_completed":invoked,"visible_writable_observed":saw_writable,
        "menu_observation_complete":discovery_complete,"post_edit_observation_complete":post_observation_complete,
        "complete":budget.reason().is_none() && show_result==Some(true) && (discovery_complete || (invoked&&post_observation_complete)),
        "stop_reason":budget.reason(),"elapsed_ms":budget.shared.started.elapsed().as_millis(),"observed_at_ms":now_ms()}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_edit_caption;

    #[test]
    fn accepts_only_the_exact_edit_address_caption_with_menu_mnemonics() {
        for caption in [
            "编辑地址",
            "编辑地址(&E)",
            "编辑地址(E)\tAlt+D",
            "&Edit address",
            "Edit address (&E)",
        ] {
            assert!(is_edit_caption(caption), "{caption}");
        }
    }

    #[test]
    fn rejects_other_commands_and_caption_substrings() {
        for caption in [
            "",
            "编辑",
            "打开",
            "复制地址",
            "Copy address",
            "Edit address settings",
            "编辑地址栏",
            "C:\\编辑地址",
            "编辑地址(AB)",
        ] {
            assert!(!is_edit_caption(caption), "{caption}");
        }
    }
}
