//! Owned Open/Save acceptance runner using the actual product adapter/worker.
//! Native IFileDialog is setup, independent truth and teardown only.
#![allow(clippy::print_stdout)]
use pathwarp::core::navigation::{Outcome, Report, Request};
use pathwarp::os::dialog::{self, Navigator};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{E_ABORT, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FOS_DONTADDTORECENT, FOS_FORCEFILESYSTEM, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileDialogEvents, IFileDialogEvents_Impl, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
    IsWindowVisible, KillTimer, PostMessageW, SetForegroundWindow, SetTimer, WM_CLOSE,
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
    static FILE_OK_COUNT: Cell<u32> = const { Cell::new(0) };
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
    let foreground = unsafe { GetForegroundWindow() };
    let mut foreground_pid = 0;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut foreground_pid)) };
    let foreground_root = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetAncestor(
            foreground,
            windows::Win32::UI::WindowsAndMessaging::GA_ROOT,
        )
    };
    Ok(json!({
        "event": "folder_snapshot",
        "hwnd": window.0 as usize,
        "folder": folder(dialog)?.replace(root.to_string_lossy().as_ref(), "PROTOTYPE_ROOT"),
        "filename": scrub(&text(unsafe { dialog.GetFileName()? })?, root),
        "file_ok_attempts": FILE_OK_COUNT.with(Cell::get),
        "file_type": unsafe { dialog.GetFileTypeIndex()? },
        "dialog_visible": unsafe { IsWindowVisible(window).as_bool() },
        "dialog_alive": unsafe { IsWindow(Some(window)).as_bool() },
        "foreground_is_dialog": foreground == window,
        "foreground_root_is_dialog": foreground_root == window && foreground_pid == unsafe { GetCurrentProcessId() },
        "foreground_hwnd": foreground.0 as usize,
        "foreground_pid": foreground_pid,
        "observed_at_ms": now_ms(),
    }))
}

#[implement(IFileDialogEvents)]
struct FolderObserver {
    root: PathBuf,
}

impl IFileDialogEvents_Impl for FolderObserver_Impl {
    fn OnFileOk(&self, dialog: Ref<IFileDialog>) -> windows::core::Result<()> {
        FILE_OK_COUNT.with(|count| count.set(count.get() + 1));
        let state = dialog
            .ok()
            .and_then(|dialog| snapshot(dialog, &self.root))
            .ok();
        println!("{}", json!({"event":"file_accept_attempt", "state":state}));
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
            "PathWarp navigation acceptance ({mode})"
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

fn scrub(value: &str, root: &Path) -> String {
    let mut out = value.replace(root.to_string_lossy().as_ref(), "PROTOTYPE_ROOT");
    if let Some(name) = root.file_name().and_then(|s| s.to_str()) {
        out = out.replace(name, "PROTOTYPE_ROOT");
    }
    if out.contains(":\\") || out.starts_with("\\\\") {
        return "NON_FIXTURE_PATH".to_owned();
    }
    if let Ok(name) = std::env::var("USERNAME")
        && out == name
    {
        return "USER_NAME".to_owned();
    }
    out
}

fn main() -> AnyResult<()> {
    if dialog::run_worker_if_requested() {
        return Ok(());
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 4 && args[1] == "--host" {
        return host(&args[2], Path::new(&args[3]));
    }
    if args.len() != 3 || args[1] != "--case" || !matches!(args[2].as_str(), "open" | "save") {
        return Err("usage: navigation_acceptance --case open|save".into());
    }
    run_case(&args[2])
}
struct OwnedHost {
    child: Child,
    receiver: mpsc::Receiver<Value>,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Drop for OwnedHost {
    fn drop(&mut self) {
        if let Some(input) = &mut self.child.stdin {
            let _ = writeln!(input, "close");
        }
        let started = Instant::now();
        while self.child.try_wait().ok().flatten().is_none() {
            if started.elapsed() >= Duration::from_secs(5) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
fn wait_result(
    navigator: &mut Navigator,
    request: Request,
    host: &OwnedHost,
    states: &mut Vec<Value>,
) -> AnyResult<Report> {
    if !navigator.start(request.clone()) {
        return Err("supervisor unexpectedly busy".into());
    }
    let started = Instant::now();
    loop {
        states.extend(host.receiver.try_iter());
        navigator.fence(
            Some(request.session),
            unsafe { GetForegroundWindow() }.0 as isize,
        );
        if let Some(report) = navigator.poll() {
            return Ok(report);
        }
        if started.elapsed() >= Duration::from_secs(18) {
            return Err("acceptance supervisor did not return".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn scrub_report(value: &mut Value, root: &Path) {
    match value {
        Value::String(text) => {
            *text = text.replace(root.to_string_lossy().as_ref(), "FIXTURE_ROOT")
        }
        Value::Array(items) => {
            for item in items {
                scrub_report(item, root);
            }
        }
        Value::Object(items) => {
            for item in items.values_mut() {
                scrub_report(item, root);
            }
        }
        _ => {}
    }
}
fn run_case(mode: &str) -> AnyResult<()> {
    let root =
        std::env::temp_dir().join(format!("PathWarp-acceptance-{}-{mode}", std::process::id()));
    std::fs::create_dir(&root)?;
    for name in ["A", "B", "A/marker-only-in-A", "B/marker-only-in-B"] {
        std::fs::create_dir(root.join(name))?;
    }
    let result = (|| -> AnyResult<()> {
        let mut child = Command::new(std::env::current_exe()?)
            .args(["--host", mode])
            .arg(&root)
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("host stdout missing")?;
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && sender.send(value).is_err()
                {
                    break;
                }
            }
        });
        let owned = OwnedHost {
            child,
            receiver,
            reader: Some(reader),
        };
        let mut states = Vec::new();
        let started = Instant::now();
        let window = loop {
            states.extend(owned.receiver.try_iter());
            if let Some(window) = states.iter().find_map(|state| {
                state["hwnd"]
                    .as_u64()
                    .filter(|_| state["dialog_visible"] == true)
            }) {
                break isize::try_from(window)?;
            }
            if started.elapsed() >= Duration::from_secs(15) {
                return Err("owned host did not become ready".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        // Normal activation of the test's own window only. No AttachThreadInput or input synthesis.
        let activation_requested = unsafe { SetForegroundWindow(HWND(window as *mut _)) }.as_bool();
        let activation_started = Instant::now();
        while unsafe { GetForegroundWindow() }.0 as isize != window {
            if activation_started.elapsed() >= Duration::from_secs(120) {
                println!(
                    "{}",
                    json!({"case":mode,"activation_requested":activation_requested,"expected_hwnd":window,"actual_foreground":unsafe{GetForegroundWindow()}.0 as usize})
                );
                return Err(
                    "normal owned-fixture activation denied; no navigation selected".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let identity = dialog::identity(window).ok_or("owned dialog identity unavailable")?;
        std::thread::sleep(Duration::from_millis(500));
        let base = Request {
            id: 1,
            session: 1,
            dialog: identity,
            overlay_hwnd: 0,
            target: root.join("A").to_string_lossy().into_owned(),
        };
        let product = std::env::current_exe()?
            .parent()
            .ok_or("executable directory missing")?
            .join("PathWarp.exe");
        let mut navigator = Navigator::with_worker(product);
        let noop = wait_result(&mut navigator, base.clone(), &owned, &mut states)?;
        let mut logged = serde_json::to_value(&noop)?;
        scrub_report(&mut logged, &root);
        println!(
            "{}",
            json!({"case":mode,"phase":"selected_type_preflight_noop","report":logged})
        );
        if noop.outcome != Outcome::AlreadyAtTarget || noop.writes != 0 || noop.invokes != 0 {
            return Err(format!(
                "type/no-op gate failed: {:?}: {}",
                noop.outcome, noop.reason
            )
            .into());
        }
        if noop.filename.as_deref() != Some("pending-prototype-name.csv") {
            return Err(
                "fixture pending filename changed during setup; no navigation selected".into(),
            );
        }
        let selected = noop
            .file_type
            .as_deref()
            .ok_or("type observation missing")?;
        if !matches!(selected, "CSV" | "CSV (*.csv)") {
            return Err(format!(
                "external type does not identify fixture CSV/index2: {selected:?}"
            )
            .into());
        }
        let missing = Request {
            id: 2,
            target: root
                .join("missing-directory")
                .to_string_lossy()
                .into_owned(),
            ..base.clone()
        };
        let unsupported = wait_result(&mut navigator, missing, &owned, &mut states)?;
        if unsupported.outcome != Outcome::Unsupported
            || unsupported.writes != 0
            || unsupported.invokes != 0
        {
            return Err("missing target did not stop before mutations".into());
        }
        let jump = Request {
            id: 3,
            target: root.join("B").to_string_lossy().into_owned(),
            ..base
        };
        let report = wait_result(&mut navigator, jump, &owned, &mut states)?;
        let final_since = now_ms();
        let mut logged = serde_json::to_value(&report)?;
        scrub_report(&mut logged, &root);
        println!(
            "{}",
            json!({"case":mode,"phase":"jump_result","report":logged})
        );
        if report.outcome != Outcome::Jumped
            || report.writes != 2
            || report.invokes != 1
            || report.filename.as_deref() != Some("pending-prototype-name.csv")
            || report.file_type.as_deref() != Some(selected)
        {
            return Err(format!(
                "full-chain gate failed: {:?}: {}",
                report.outcome, report.reason
            )
            .into());
        }
        let started = Instant::now();
        loop {
            states.extend(owned.receiver.try_iter());
            let fresh: Vec<_> = states
                .iter()
                .filter(|state| {
                    state["event"] == "host_poll"
                        && state["observed_at_ms"]
                            .as_u64()
                            .is_some_and(|time| u128::from(time) >= final_since)
                })
                .collect();
            if fresh.len() >= 6 {
                let preserved = fresh.iter().all(|state| {
                    state["folder"] == "PROTOTYPE_ROOT\\B"
                        && state["filename"] == "pending-prototype-name.csv"
                        && state["file_type"] == 2
                        && state["hwnd"]
                            .as_u64()
                            .and_then(|hwnd| isize::try_from(hwnd).ok())
                            == Some(window)
                        && state["dialog_alive"] == true
                        && state["dialog_visible"] == true
                        && state["file_ok_attempts"] == 0
                });
                if !preserved
                    || states
                        .iter()
                        .any(|state| state["event"] == "file_accept_attempt")
                {
                    return Err("native pending-operation preservation failed".into());
                }
                println!(
                    "{}",
                    json!({"case":mode,"passed":true,"selected_type":selected,"native_type_index":2,"fresh_native_samples":fresh.len(),"no_op_zero_actions":true,"missing_directory_zero_actions":true,"native_final_samples":fresh})
                );
                break;
            }
            if started.elapsed() >= Duration::from_secs(3) {
                return Err("fresh native samples missing".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    })();
    let mut cleaned = true;
    for path in [
        root.join("A/marker-only-in-A"),
        root.join("B/marker-only-in-B"),
        root.join("A"),
        root.join("B"),
        root.clone(),
    ] {
        cleaned &= std::fs::remove_dir(path).is_ok();
    }
    println!(
        "{}",
        json!({"case":mode,"cleanup":cleaned,"error":result.as_ref().err().map(ToString::to_string)})
    );
    result
}
