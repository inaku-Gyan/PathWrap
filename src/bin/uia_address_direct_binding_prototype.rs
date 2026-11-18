//! THROWAWAY: one bounded initial-A direct pattern-acquisition pass.
//! Run: cargo run --bin uia_address_direct_binding_prototype
//! No provider actions, setters, focus changes or input simulation.
//! Derived from the accepted read-only inventory, retained unchanged beside this file.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{E_ABORT, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::System::Variant::{VT_BOOL, VT_BSTR};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
    IUIAutomationLegacyIAccessiblePattern, IUIAutomationTreeWalker, IUIAutomationValuePattern,
    UIA_PATTERN_ID, UIA_PROPERTY_ID,
};
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
use windows::core::{BOOL, HSTRING, IUnknown, Interface, PWSTR, Ref, implement, w};

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
        return Err("usage: uia_address_direct_binding_prototype".into());
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
            "PROTOTYPE — direct address pattern acquisition ({mode})"
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
        "PathWarp-DIRECTBINDING-PROTOTYPE-{}-{mode}",
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

// Fixed windows 0.62.2 catalog, cross-checked against the published research.
// These are defined pairs, not a scan of an arbitrary numeric interval.
const CATALOG: [(&str, i32, i32); 35] = [
    ("InvokePattern", 10000, 30031),
    ("SelectionPattern", 10001, 30037),
    ("ValuePattern", 10002, 30043),
    ("RangeValuePattern", 10003, 30033),
    ("ScrollPattern", 10004, 30034),
    ("ExpandCollapsePattern", 10005, 30028),
    ("GridPattern", 10006, 30030),
    ("GridItemPattern", 10007, 30029),
    ("MultipleViewPattern", 10008, 30032),
    ("WindowPattern", 10009, 30044),
    ("SelectionItemPattern", 10010, 30036),
    ("DockPattern", 10011, 30027),
    ("TablePattern", 10012, 30038),
    ("TableItemPattern", 10013, 30039),
    ("TextPattern", 10014, 30040),
    ("TogglePattern", 10015, 30041),
    ("TransformPattern", 10016, 30042),
    ("ScrollItemPattern", 10017, 30035),
    ("LegacyIAccessiblePattern", 10018, 30090),
    ("ItemContainerPattern", 10019, 30108),
    ("VirtualizedItemPattern", 10020, 30109),
    ("SynchronizedInputPattern", 10021, 30110),
    ("ObjectModelPattern", 10022, 30112),
    ("AnnotationPattern", 10023, 30118),
    ("TextPattern2", 10024, 30119),
    ("StylesPattern", 10025, 30127),
    ("SpreadsheetPattern", 10026, 30128),
    ("SpreadsheetItemPattern", 10027, 30132),
    ("TransformPattern2", 10028, 30134),
    ("TextChildPattern", 10029, 30136),
    ("DragPattern", 10030, 30137),
    ("DropTargetPattern", 10031, 30141),
    ("TextEditPattern", 10032, 30149),
    ("CustomNavigationPattern", 10033, 30151),
    ("SelectionPattern2", 10034, 30168),
];

struct Budget {
    started: Instant,
    calls: u32,
    stop: Option<String>,
    read_errors: u32,
    window: HWND,
    host_pid: u32,
}

impl Budget {
    fn new(window: HWND, host_pid: u32) -> Self {
        Self {
            started: Instant::now(),
            calls: 0,
            stop: None,
            read_errors: 0,
            window,
            host_pid,
        }
    }

    fn stop(&mut self, reason: &str) {
        if self.stop.is_none() {
            self.stop = Some(reason.to_owned());
            println!(
                "{}",
                json!({"event":"pass_stop", "reason":reason, "calls":self.calls,
                "elapsed_ms":self.started.elapsed().as_millis(), "observed_at_ms":now_ms()})
            );
        }
    }

    fn open(&mut self) -> bool {
        if self.stop.is_none() {
            // Read-only native guards do not count as explicit UIA calls.
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(self.window, Some(&mut pid)) };
            if pid != self.host_pid || !unsafe { IsWindow(Some(self.window)).as_bool() } {
                self.stop("original_dialog_identity_lost");
            }
        }
        if self.stop.is_none() && self.calls >= 4096 {
            self.stop("call_budget");
        }
        if self.stop.is_none() && self.started.elapsed() >= Duration::from_secs(10) {
            self.stop("soft_time_budget");
        }
        self.stop.is_none()
    }

    fn query<T>(
        &mut self,
        label: &str,
        query: impl FnOnce() -> windows::core::Result<T>,
    ) -> Option<windows::core::Result<T>> {
        if !self.open() {
            return None;
        }
        self.calls += 1;
        let call = self.calls;
        println!(
            "{}",
            json!({"event":"read_start", "call":call,"label":label,"observed_at_ms":now_ms()})
        );
        let _ = std::io::stdout().flush();
        let started = Instant::now();
        let result = query();
        if result.is_err() {
            self.read_errors += 1;
        }
        println!(
            "{}",
            json!({"event":"read_end", "call":call,"label":label,
            "rust_result":if result.is_ok() {"ok"} else {"err"},
            "rust_error_code":result.as_ref().err().map(|error| format!("{:?}",error.code())),
            "raw_hresult":null, "elapsed_us":started.elapsed().as_micros(),"observed_at_ms":now_ms()})
        );
        Some(result)
    }
}

fn outcome<T>(value: &Option<windows::core::Result<T>>, render: impl FnOnce(&T) -> Value) -> Value {
    match value {
        None => json!({"status":"not_requested"}),
        Some(Ok(value)) => json!({"status":"ok", "value":render(value)}),
        Some(Err(error)) => {
            json!({"status":"error", "rust_error_code":format!("{:?}",error.code())})
        }
    }
}

fn binding_outcome<T>(value: &Option<windows::core::Result<T>>) -> Value {
    match value {
        Some(Err(error)) if error.code().is_ok() => json!({
            "status":"binding_null_unknown", "rust_error_code":format!("{:?}",error.code()),
            "raw_hresult":null, "note":"High-level binding did not preserve raw success HRESULT/pointer."}),
        _ => outcome(value, |_| json!("interface_obtained")),
    }
}

fn field<T>(
    budget: &mut Budget,
    label: &str,
    query: impl FnOnce() -> windows::core::Result<T>,
    render: impl FnOnce(&T) -> Value,
) -> Value {
    outcome(&budget.query(label, query), render)
}

fn property(
    budget: &mut Budget,
    uia: &IUIAutomation,
    element: &IUIAutomationElement,
    label: &str,
    id: i32,
    expected_bool: bool,
    root: &Path,
) -> Value {
    let result = budget.query(label, || unsafe {
        element.GetCurrentPropertyValueEx(UIA_PROPERTY_ID(id), true)
    });
    let Some(Ok(value)) = result.as_ref() else {
        return outcome(&result, |_| Value::Null);
    };
    let variant_type = value.vt().0;
    let check = budget.query(&format!("{label}.CheckNotSupported"), || unsafe {
        uia.CheckNotSupported(value)
    });
    match check {
        None => {
            return json!({"status":"incomplete", "variant_type":variant_type,"check":"not_requested"});
        }
        Some(Err(error)) => {
            return json!({"status":"unknown", "variant_type":variant_type,
            "check_error":format!("{:?}",error.code())});
        }
        Some(Ok(is_not_supported)) if is_not_supported.as_bool() => {
            return json!({"status":"not_supported", "variant_type":variant_type});
        }
        _ => {}
    }
    // Access only the union member selected by the exact VARIANT tag.
    if expected_bool && value.vt() == VT_BOOL {
        let value = unsafe { value.Anonymous.Anonymous.Anonymous.boolVal.as_bool() };
        json!({"status":"bool", "variant_type":variant_type,"value":value})
    } else if !expected_bool && value.vt() == VT_BSTR {
        let value = unsafe { &value.Anonymous.Anonymous.Anonymous.bstrVal };
        json!({"status":"string", "variant_type":variant_type,"value":scrub(&value.to_string(),root)})
    } else {
        json!({"status":"type_error", "variant_type":variant_type})
    }
}

fn legacy_details(budget: &mut Budget, unknown: &IUnknown, prefix: &str, root: &Path) -> Value {
    let binding = budget.query(&format!("{prefix}.QueryInterface.Legacy"), || {
        unknown.cast::<IUIAutomationLegacyIAccessiblePattern>()
    });
    let status = binding_outcome(&binding);
    let Some(Ok(pattern)) = binding else {
        return json!({"binding":status});
    };
    macro_rules! text_field {
        ($method:ident) => {
            field(
                budget,
                &format!("{prefix}.{}", stringify!($method)),
                || unsafe { pattern.$method() },
                |value| json!(scrub(&value.to_string(), root)),
            )
        };
    }
    let name = text_field!(CurrentName);
    let description = text_field!(CurrentDescription);
    let help = text_field!(CurrentHelp);
    let value = text_field!(CurrentValue);
    let action = text_field!(CurrentDefaultAction);
    let shortcut = text_field!(CurrentKeyboardShortcut);
    let role = field(
        budget,
        &format!("{prefix}.CurrentRole"),
        || unsafe { pattern.CurrentRole() },
        |value| json!(value),
    );
    let state = field(
        budget,
        &format!("{prefix}.CurrentState"),
        || unsafe { pattern.CurrentState() },
        |value| json!({"raw":value,"hex":format!("0x{value:08X}")}),
    );
    let child_id = field(
        budget,
        &format!("{prefix}.CurrentChildId"),
        || unsafe { pattern.CurrentChildId() },
        |value| json!(value),
    );
    json!({"binding":status,"name":name,"description":description,"help":help,
        "value":value,"default_action":action,"keyboard_shortcut":shortcut,"role":role,"state":state,"child_id":child_id})
}

fn pattern_details(
    budget: &mut Budget,
    unknown: &IUnknown,
    id: i32,
    prefix: &str,
    root: &Path,
) -> Value {
    match id {
        10018 => legacy_details(budget, unknown, prefix, root),
        10002 => {
            let binding = budget.query(&format!("{prefix}.QueryInterface.Value"), || {
                unknown.cast::<IUIAutomationValuePattern>()
            });
            let status = binding_outcome(&binding);
            let Some(Ok(pattern)) = binding else {
                return json!({"binding":status});
            };
            let value = field(
                budget,
                &format!("{prefix}.CurrentValue"),
                || unsafe { pattern.CurrentValue() },
                |value| json!(scrub(&value.to_string(), root)),
            );
            let readonly = field(
                budget,
                &format!("{prefix}.CurrentIsReadOnly"),
                || unsafe { pattern.CurrentIsReadOnly() },
                |value| json!(value.as_bool()),
            );
            json!({"binding":status,"value":value,"readonly":readonly})
        }
        10005 => {
            let binding = budget.query(&format!("{prefix}.QueryInterface.ExpandCollapse"), || {
                unknown.cast::<IUIAutomationExpandCollapsePattern>()
            });
            let status = binding_outcome(&binding);
            let Some(Ok(pattern)) = binding else {
                return json!({"binding":status});
            };
            let state = field(
                budget,
                &format!("{prefix}.CurrentExpandCollapseState"),
                || unsafe { pattern.CurrentExpandCollapseState() },
                |value| json!(value.0),
            );
            json!({"binding":status,"state":state})
        }
        _ => json!({"status":"no_getters_in_whitelist"}),
    }
}

fn capabilities(
    budget: &mut Budget,
    uia: &IUIAutomation,
    element: &IUIAutomationElement,
    index: usize,
    root: &Path,
) -> Value {
    let prefix = format!("node_{index}");
    macro_rules! attr {
        ($method:ident,$render:expr) => {
            field(
                budget,
                &format!("{prefix}.{}", stringify!($method)),
                || unsafe { element.$method() },
                $render,
            )
        };
    }
    let attributes = json!({
        "name":attr!(CurrentName,|v|json!(scrub(&v.to_string(),root))),
        "class":attr!(CurrentClassName,|v|json!(v.to_string())),
        "automation_id":attr!(CurrentAutomationId,|v|json!(v.to_string())),
        "control_type":attr!(CurrentControlType,|v|json!(v.0)),
        "framework":attr!(CurrentFrameworkId,|v|json!(v.to_string())),
        "provider":attr!(CurrentProviderDescription,|v|json!(scrub(&v.to_string(),root))),
        "native_hwnd":attr!(CurrentNativeWindowHandle,|v|json!(v.0 as usize)),
        "enabled":attr!(CurrentIsEnabled,|v|json!(v.as_bool())),
        "offscreen":attr!(CurrentIsOffscreen,|v|json!(v.as_bool())),
        "focusable":attr!(CurrentIsKeyboardFocusable,|v|json!(v.as_bool())),
        "has_focus":attr!(CurrentHasKeyboardFocus,|v|json!(v.as_bool())),
        "bounds":attr!(CurrentBoundingRectangle,|v|json!({"left":v.left,"top":v.top,"right":v.right,"bottom":v.bottom})),
        "access_key":property(budget,uia,element,&format!("{prefix}.AccessKey"),30007,false,root),
        "accelerator_key":property(budget,uia,element,&format!("{prefix}.AcceleratorKey"),30006,false,root),
    });
    let mut patterns = Vec::new();
    for (name, id, availability_id) in CATALOG {
        let label = format!("{prefix}.{name}");
        let availability = property(
            budget,
            uia,
            element,
            &format!("{label}.availability"),
            availability_id,
            true,
            root,
        );
        // Acquisition is independent of the preceding availability observation.
        // Replace the former positive-only gate; never bind a cell twice.
        let acquired = budget.query(&format!("{label}.GetCurrentPattern"), || unsafe {
            element.GetCurrentPattern(UIA_PATTERN_ID(id))
        });
        let binding = binding_outcome(&acquired);
        let details = if let Some(Ok(unknown)) = acquired {
            pattern_details(budget, &unknown, id, &label, root)
        } else {
            json!({"status":"not_requested","reason":"interface_not_obtained"})
        };
        patterns.push(
            json!({"name":name,"pattern_id":id,"availability_property_id":availability_id,
            "availability":availability,"binding":binding,"details":details}),
        );
    }
    json!({"attributes":attributes,"patterns":patterns})
}

fn address_windows(window: HWND) -> Vec<HWND> {
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
        unsafe { &mut *(data.0 as *mut Vec<HWND>) }.push(window);
        BOOL(1)
    }
    let mut windows = Vec::new();
    let _ = unsafe {
        EnumChildWindows(
            Some(window),
            Some(collect),
            LPARAM((&raw mut windows) as isize),
        )
    };
    windows
}

fn child_result(
    budget: &mut Budget,
    label: &str,
    query: impl FnOnce() -> windows::core::Result<IUIAutomationElement>,
) -> Option<IUIAutomationElement> {
    match budget.query(label, query) {
        Some(Ok(element)) => Some(element),
        Some(Err(error)) if error.code().is_ok() => {
            println!(
                "{}",
                json!({"event":"tree_end_binding_null","label":label,"raw_hresult":null})
            );
            None
        }
        Some(Err(_)) => {
            budget.stop("tree_read_error");
            None
        }
        None => None,
    }
}

struct Capture {
    seen: Vec<IUIAutomationElement>,
    edges: usize,
    roots_done: usize,
    host_pid: u32,
}

impl Capture {
    fn walk(
        &mut self,
        budget: &mut Budget,
        uia: &IUIAutomation,
        walker: &IUIAutomationTreeWalker,
        seed: IUIAutomationElement,
        source: &str,
        root: &Path,
    ) {
        let mut pending = vec![(seed, None::<usize>, 0_usize)];
        while let Some((element, parent, depth)) = pending.pop() {
            if !budget.open() {
                break;
            }
            if depth > 16 {
                budget.stop("depth_budget");
                break;
            }
            let mut duplicate = None;
            for (index, existing) in self.seen.iter().enumerate() {
                match budget.query("CompareElements.deduplicate", || unsafe {
                    uia.CompareElements(existing, &element)
                }) {
                    Some(Ok(same)) if same.as_bool() => {
                        duplicate = Some(index);
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => {
                        budget.stop("identity_comparison_error");
                        break;
                    }
                    None => break,
                }
            }
            if !budget.open() {
                break;
            }
            self.edges += 1;
            if let Some(index) = duplicate {
                println!(
                    "{}",
                    json!({"event":"duplicate_edge","source":source,"parent":parent,"depth":depth,"index":index})
                );
                continue;
            }
            if self.seen.len() >= 64 {
                budget.stop("element_budget");
                break;
            }
            let index = self.seen.len();
            let pid = budget.query(&format!("node_{index}.CurrentProcessId"), || unsafe {
                element.CurrentProcessId()
            });
            if !matches!(pid,Some(Ok(pid)) if i64::from(pid)==i64::from(self.host_pid)) {
                budget.stop("element_process_identity_unavailable_or_mismatch");
                break;
            }
            self.seen.push(element.clone());
            println!(
                "{}",
                json!({"event":"node_start","index":index,"parent":parent,"depth":depth,"source":source,"pid":self.host_pid,"observed_at_ms":now_ms()})
            );
            let data = capabilities(budget, uia, &element, index, root);
            println!(
                "{}",
                json!({"event":"capability_node","index":index,"parent":parent,"depth":depth,"source":source,
                "pid":self.host_pid,"data":data,"observed_at_ms":now_ms()})
            );
            if !budget.open() {
                break;
            }
            let mut children = Vec::new();
            let mut child = child_result(budget, "RawViewWalker.GetFirstChild", || unsafe {
                walker.GetFirstChildElement(&element)
            });
            while let Some(current) = child {
                if !budget.open() {
                    break;
                }
                if children.len() >= 64 {
                    budget.stop("pending_children_budget");
                    break;
                }
                child = child_result(budget, "RawViewWalker.GetNextSibling", || unsafe {
                    walker.GetNextSiblingElement(&current)
                });
                children.push(current);
            }
            for child in children.into_iter().rev() {
                pending.push((child, Some(index), depth + 1));
            }
        }
        println!(
            "{}",
            json!({"event":"source_capture_end","source":source,"pending_known_nodes":pending.len(),"budget_open":budget.open(),"observed_at_ms":now_ms()})
        );
        if budget.stop.is_none() {
            self.roots_done += 1;
        }
    }
}

fn observe_focus(
    budget: &mut Budget,
    uia: &IUIAutomation,
    root: &Path,
    host_pid: u32,
    phase: &str,
) {
    let focus = budget.query(&format!("{phase}.GetFocusedElement"), || unsafe {
        uia.GetFocusedElement()
    });
    let mut data = binding_outcome(&focus);
    if let Some(Ok(element)) = focus {
        let pid = budget.query(&format!("{phase}.focus.CurrentProcessId"), || unsafe {
            element.CurrentProcessId()
        });
        data = json!({"pid":outcome(&pid,|pid|json!(pid))});
        if matches!(pid,Some(Ok(pid)) if i64::from(pid)==i64::from(host_pid)) {
            data["name"] = field(
                budget,
                &format!("{phase}.focus.CurrentName"),
                || unsafe { element.CurrentName() },
                |v| json!(scrub(&v.to_string(), root)),
            );
            data["class"] = field(
                budget,
                &format!("{phase}.focus.CurrentClassName"),
                || unsafe { element.CurrentClassName() },
                |v| json!(v.to_string()),
            );
        }
    }
    println!(
        "{}",
        json!({"event":"focus_foreground","phase":phase,"focused":data,
        "foreground":native_info(unsafe{GetForegroundWindow()}),"observed_at_ms":now_ms()})
    );
}

fn probe(window: HWND, root: &Path, host_pid: u32) -> AnyResult<()> {
    println!(
        "{}",
        json!({"event":"uia_client_start","client_pid":std::process::id(),"host_pid":host_pid})
    );
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let mut budget = Budget::new(window, host_pid);
    println!(
        "{}",
        json!({"event":"read_pass_start","observed_at_ms":now_ms(),
        "catalog":"windows-0.62.2 / windows-rs 32c3144490c016fe496a0aed769bce60987a2e9d",
        "binding_policy":"once_per_catalog_entry_independent_of_availability","native_target_guard":true,
        "patterns":CATALOG.as_slice(),"max_elements":64,"max_depth":16,"max_explicit_calls":4096,"soft_budget_ms":10000,
        "raw_hresult_retained":false,"call_count_note":"Every explicit UIA query/walk/compare/bind/getter is counted; native identity guards, internal RPC and interface releases are not separately counted."})
    );
    let uia: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let bands: Vec<_> = address_windows(window)
        .into_iter()
        .filter(|hwnd| {
            let native = native_info(*hwnd);
            native["class"] == "Address Band Root" && native["pid"] == host_pid
        })
        .collect();
    if bands.len() != 1 || !unsafe { IsWindow(Some(window)).as_bool() } {
        budget.stop("address_band_not_unique_or_original_dialog_lost");
    }
    let mut capture = Capture {
        seen: Vec::new(),
        edges: 0,
        roots_done: 0,
        host_pid,
    };
    let mut planned_roots = 0;
    if budget.open() {
        let band_window = bands[0];
        let descendants = address_windows(band_window);
        let mut native_seeds = Vec::new();
        for class in ["ComboBoxEx32", "ComboBox", "Edit"] {
            let matches: Vec<_> = descendants
                .iter()
                .copied()
                .filter(|hwnd| {
                    let native = native_info(*hwnd);
                    native["class"] == class
                        && native["pid"] == host_pid
                        && unsafe { IsChild(band_window, *hwnd).as_bool() }
                })
                .collect();
            println!(
                "{}",
                json!({"event":"native_seed_discovery","class":class,"count":matches.len(),"observed_at_ms":now_ms()})
            );
            if matches.len() != 1 {
                budget.stop("hidden_address_seed_not_unique");
                break;
            }
            native_seeds.push((matches[0], class));
        }
        if budget.open() {
            observe_focus(&mut budget, &uia, root, host_pid, "before");
            let walker = budget.query("RawViewWalker", || unsafe { uia.RawViewWalker() });
            let mut seeds = vec![(band_window, "address_band")];
            seeds.extend(native_seeds.iter().copied());
            planned_roots = seeds.len();
            if let Some(Ok(walker)) = walker {
                for (seed, source) in seeds {
                    if !budget.open() {
                        break;
                    }
                    if !unsafe { IsWindow(Some(window)).as_bool() }
                        || (seed != band_window && !unsafe { IsChild(band_window, seed).as_bool() })
                    {
                        budget.stop("original_target_identity_lost");
                        break;
                    }
                    println!(
                        "{}",
                        json!({"event":"address_hwnd","phase":"before_source","source":source,"state":native_info(seed),"observed_at_ms":now_ms()})
                    );
                    let element = budget.query(&format!("{source}.ElementFromHandle"), || unsafe {
                        uia.ElementFromHandle(seed)
                    });
                    let Some(Ok(element)) = element else {
                        budget.stop("seed_element_unavailable");
                        break;
                    };
                    let actual_hwnd = budget.query("seed.CurrentNativeWindowHandle", || unsafe {
                        element.CurrentNativeWindowHandle()
                    });
                    if !matches!(actual_hwnd,Some(Ok(actual)) if actual==seed) {
                        budget.stop("seed_native_binding_mismatch");
                        break;
                    }
                    capture.walk(&mut budget, &uia, &walker, element, source, root);
                }
            } else {
                budget.stop("raw_walker_unavailable");
            }
            for (hwnd, source) in native_seeds {
                println!(
                    "{}",
                    json!({"event":"address_hwnd","phase":"after_pass","source":source,"state":native_info(hwnd),"observed_at_ms":now_ms()})
                );
            }
            if budget.open() {
                observe_focus(&mut budget, &uia, root, host_pid, "after");
            }
        }
    }
    let budget_open = budget.open();
    println!(
        "{}",
        json!({"event":"read_pass_done","calls":budget.calls,"unique_elements":capture.seen.len(),
        "source_edges":capture.edges,"planned_roots":planned_roots,"roots_done":capture.roots_done,
        "complete":budget_open && capture.roots_done==planned_roots && planned_roots==4,
        "stop_reason":budget.stop,"rust_read_err_results":budget.read_errors,
        "elapsed_ms":budget.started.elapsed().as_millis(),"provider_actions":0,"observed_at_ms":now_ms()})
    );
    Ok(())
}
