//! THROWAWAY: filename Value SetValue -> IDOK Invoke -> observed B -> restore filename.
//! One owned case per invocation; no synthetic input, fallback or native navigation.
#![allow(clippy::print_stdout)]
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{E_ABORT, E_FAIL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationValuePattern, TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_InvokePatternId,
    UIA_SplitButtonControlTypeId, UIA_ValuePatternId,
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
    EnumChildWindows, EnumWindows, GetClassNameW, GetForegroundWindow, GetParent,
    GetWindowThreadProcessId, IsChild, IsWindow, IsWindowVisible, KillTimer, PostMessageW,
    SetTimer, WM_CLOSE,
};
use windows::core::{BOOL, BSTR, HSTRING, Interface, PWSTR, Ref, implement, w};

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

fn main() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--host") => host(&args[2], Path::new(&args[3])),
        Some("--client") => probe(
            HWND(args[2].parse::<usize>()? as *mut _),
            Path::new(&args[3]),
            args[4].parse()?,
        ),
        Some("--case") if args.len() == 3 && matches!(args[2].as_str(), "open" | "save") => {
            run(&args[2])
        }
        _ => Err("usage: uia_filename_folder_jump_prototype --case open|save".into()),
    }
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
        "filename": scrub(&text(unsafe { dialog.GetFileName()? })?, root),
        "file_ok_attempts": FILE_OK_COUNT.with(Cell::get),
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
            "PROTOTYPE — filename UIA folder jump ({mode})"
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

fn native(window: HWND) -> Value {
    let mut pid = 0;
    let mut class = [0_u16; 128];
    let len = unsafe { GetClassNameW(window, &mut class) };
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    json!({"hwnd":window.0 as usize, "pid":pid, "class":String::from_utf16_lossy(&class[..usize::try_from(len).unwrap_or(0)]),
        "alive":unsafe {IsWindow(Some(window)).as_bool()}, "visible":unsafe {IsWindowVisible(window).as_bool()}, "enabled":unsafe {IsWindowEnabled(window).as_bool()}})
}

fn run(mode: &str) -> AnyResult<()> {
    let root = std::env::temp_dir().join(format!(
        "PathWarp-FILENAME-PROTOTYPE-{}-{mode}",
        std::process::id()
    ));
    if root.exists() {
        return Err("fixture root already exists; no retry".into());
    }
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
    println!(
        "{}",
        json!({"event":"case_start", "mode":mode, "host_pid":child.id(), "supervisor_pid":std::process::id(), "observed_at_ms":now_ms()})
    );
    let result = (|| -> AnyResult<()> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let before = loop {
            let event = match receive.recv_timeout(Duration::from_millis(500)) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => continue,
                Err(error) => return Err(error.into()),
            };
            if event["event"] == "host_poll"
                && event["dialog_alive"] == true
                && event["dialog_visible"] == true
                && event["filename"] == "pending-prototype-name.csv"
                && event["file_type"] == 2
                && event["folder"] == "PROTOTYPE_ROOT\\A"
                && event["file_ok_attempts"] == 0
            {
                break event;
            }
            if Instant::now() >= deadline {
                return Err("fully initialized fixture baseline unavailable".into());
            }
        };
        let raw = before["hwnd"].as_u64().ok_or("missing HWND")?;
        println!("{}", json!({"event":"baseline", "state":before}));
        let mut client = Command::new(std::env::current_exe()?)
            .arg("--client")
            .arg(raw.to_string())
            .arg(&root)
            .arg(child.id().to_string())
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdout(Stdio::piped())
            .spawn()?;
        let output = client.stdout.take().ok_or("missing client stdout")?;
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
                client.kill()?;
                break client.wait()?;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let _ = reader.join();
        let after_start = now_ms();
        let after_deadline = Instant::now() + Duration::from_millis(600);
        let mut samples = Vec::new();
        while Instant::now() < after_deadline {
            if let Ok(event) = receive.recv_timeout(Duration::from_millis(100))
                && event["event"] == "host_poll"
                && event["observed_at_ms"]
                    .as_u64()
                    .is_some_and(|n| u128::from(n) >= after_start)
            {
                samples.push(event);
            }
        }
        let preserved_b = !samples.is_empty()
            && samples.iter().all(|s| {
                s["folder"] == "PROTOTYPE_ROOT\\B"
                    && s["filename"] == before["filename"]
                    && s["file_type"] == before["file_type"]
                    && s["hwnd"] == before["hwnd"]
                    && s["dialog_alive"] == true
                    && s["dialog_visible"] == true
                    && s["file_ok_attempts"] == 0
            });
        println!(
            "{}",
            json!({"event":"native_verdict", "before":before, "fresh_final_samples":samples, "native_full_chain_preserved":preserved_b,
            "worker_success":status.success(), "timed_out":timed_out, "worker_lifetime_ms":started.elapsed().as_millis(), "worker_timeout_ms":15000,
            "note":"Native truth is an independent assertion, never an input to the worker's arrival/restoration gate."})
        );
        Ok(())
    })();
    if let Some(input) = child.stdin.as_mut() {
        let _ = writeln!(input, "close");
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let _ = host_reader.join();
    // Non-recursive removal only; these are the five directories this case created.
    let cleanup = [
        root.join("A/marker-only-in-A"),
        root.join("B/marker-only-in-B"),
        root.join("A"),
        root.join("B"),
        root.clone(),
    ]
    .iter()
    .all(|path| std::fs::remove_dir(path).is_ok());
    println!(
        "{}",
        json!({"event":"fixture_cleanup", "host_exited":true, "scratch_directories_removed":cleanup, "case_error":result.as_ref().err().map(ToString::to_string)})
    );
    result
}

thread_local! {
    static CALLS: Cell<u32> = const { Cell::new(0) };
    static WORKER_START: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn call<T>(
    operation: &str,
    action: bool,
    f: impl FnOnce() -> windows::core::Result<T>,
) -> windows::core::Result<T> {
    let id = CALLS.with(|n| {
        let next = n.get() + 1;
        n.set(next);
        next
    });
    if id > 1024
        || WORKER_START.with(|s| {
            s.get()
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(12))
        })
    {
        println!(
            "{}",
            json!({"event":"bound_hit", "operation":operation, "call_id":id})
        );
        return Err(E_FAIL.into());
    }
    let started = Instant::now();
    println!(
        "{}",
        json!({"event":"uia_call_start", "call_id":id, "operation":operation, "action":action, "observed_at_ms":now_ms()})
    );
    let result = f();
    println!(
        "{}",
        json!({"event":"uia_call_end", "call_id":id, "operation":operation, "ok":result.is_ok(), "error_code":result.as_ref().err().map(|e| format!("{:?}", e.code())), "raw_success_hresult":Value::Null, "elapsed_us":started.elapsed().as_micros(), "observed_at_ms":now_ms()})
    );
    result
}

fn descendants(
    uia: &IUIAutomation,
    anchor: &IUIAutomationElement,
    types: &[i32],
) -> AnyResult<Vec<IUIAutomationElement>> {
    let first = call("CreatePropertyCondition", false, || unsafe {
        uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(types[0]))
    })?;
    let condition = if types.len() == 2 {
        let second = call("CreatePropertyCondition", false, || unsafe {
            uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(types[1]))
        })?;
        call("CreateOrCondition", false, || unsafe {
            uia.CreateOrCondition(&first, &second)
        })?
    } else {
        first
    };
    let found = call("FindAll.typed", false, || unsafe {
        anchor.FindAll(TreeScope_Descendants, &condition)
    })?;
    let count = call("ElementArray.Length", false, || unsafe { found.Length() })?;
    if count > 64 {
        return Err("typed lookup exceeds 64-element bound".into());
    }
    let mut result = Vec::new();
    for i in 0..count {
        result.push(call("ElementArray.GetElement", false, || unsafe {
            found.GetElement(i)
        })?);
    }
    Ok(result)
}

fn ensure_target(
    uia: &IUIAutomation,
    anchor: &IUIAutomationElement,
    target: &IUIAutomationElement,
    window: HWND,
    pid: u32,
) -> AnyResult<Value> {
    let state = native(window);
    if state["pid"] != pid
        || state["alive"] != true
        || state["visible"] != true
        || state["enabled"] != true
    {
        return Err("original dialog is unavailable".into());
    }
    let actual_pid = call("CurrentProcessId", false, || unsafe {
        target.CurrentProcessId()
    })?;
    let hwnd = call("CurrentNativeWindowHandle", false, || unsafe {
        target.CurrentNativeWindowHandle()
    })?;
    let enabled = call("CurrentIsEnabled", false, || unsafe {
        target.CurrentIsEnabled()
    })?
    .as_bool();
    let offscreen = call("CurrentIsOffscreen", false, || unsafe {
        target.CurrentIsOffscreen()
    })?
    .as_bool();
    if actual_pid != i32::try_from(pid)? || !enabled || offscreen {
        return Err("target ownership/visibility/enabled gate failed".into());
    }
    if !hwnd.is_invalid()
        && (!unsafe { IsChild(window, hwnd).as_bool() }
            || native(hwnd)["pid"] != pid
            || native(hwnd)["visible"] != true
            || native(hwnd)["enabled"] != true)
    {
        return Err("target native ancestry/visibility gate failed".into());
    }
    let walker = call("RawViewWalker", false, || unsafe { uia.RawViewWalker() })?;
    let mut cursor = target.clone();
    let mut anchored = false;
    for _ in 0..16 {
        if call("CompareElements.ancestor", false, || unsafe {
            uia.CompareElements(&cursor, anchor)
        })?
        .as_bool()
        {
            anchored = true;
            break;
        }
        cursor = call("GetParentElement", false, || unsafe {
            walker.GetParentElement(&cursor)
        })?;
    }
    if !anchored {
        return Err("target is not under the original UIA dialog".into());
    }
    Ok(
        json!({"pid":actual_pid,"hwnd":hwnd.0 as usize,"enabled":enabled,"offscreen":offscreen,"ancestry_matches":true}),
    )
}

fn filename(
    uia: &IUIAutomation,
    anchor: &IUIAutomationElement,
    window: HWND,
    pid: u32,
    root: &Path,
    expected: Option<&str>,
) -> AnyResult<(IUIAutomationElement, IUIAutomationValuePattern, String)> {
    let mut matches = Vec::new();
    for edit in descendants(uia, anchor, &[UIA_EditControlTypeId.0])? {
        let id = call("CurrentAutomationId.edit", false, || unsafe {
            edit.CurrentAutomationId()
        })?
        .to_string();
        if !matches!(id.as_str(), "1148" | "1001" | "FileNameControlHost") {
            continue;
        }
        let name = call("CurrentName.edit", false, || unsafe { edit.CurrentName() })?.to_string();
        let semantic = name.contains("文件名") || name.to_lowercase().contains("file name");
        println!(
            "{}",
            json!({"event":"filename_candidate","automation_id":id,"name":scrub(&name,root),"semantic_match":semantic})
        );
        if !semantic {
            continue;
        }
        let identity = ensure_target(uia, anchor, &edit, window, pid)?;
        let value: IUIAutomationValuePattern = call("GetCurrentPattern.Value", false, || unsafe {
            edit.GetCurrentPatternAs(UIA_ValuePatternId)
        })?;
        let readonly = call("CurrentIsReadOnly", false, || unsafe {
            value.CurrentIsReadOnly()
        })?
        .as_bool();
        let current = call("CurrentValue", false, || unsafe { value.CurrentValue() })?.to_string();
        println!(
            "{}",
            json!({"event":"filename_state","identity":identity,"value":scrub(&current,root),"readonly":readonly})
        );
        if !readonly && expected.is_none_or(|text| text == current) {
            matches.push((edit, value, current));
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "filename target count is {}, expected one exact semantic/identity/value match",
            matches.len()
        )
        .into());
    }
    matches
        .pop()
        .ok_or_else(|| "filename target missing".into())
}

fn button_caption(name: &str) -> bool {
    let text = name
        .split(['(', '（', '\t'])
        .next()
        .unwrap_or_default()
        .replace('&', "")
        .trim()
        .to_lowercase();
    matches!(text.as_str(), "打开" | "保存" | "open" | "save")
}

fn button(
    uia: &IUIAutomation,
    anchor: &IUIAutomationElement,
    window: HWND,
    pid: u32,
    root: &Path,
) -> AnyResult<IUIAutomationElement> {
    let mut matches = Vec::new();
    for element in descendants(
        uia,
        anchor,
        &[UIA_ButtonControlTypeId.0, UIA_SplitButtonControlTypeId.0],
    )? {
        let id = call("CurrentAutomationId.button", false, || unsafe {
            element.CurrentAutomationId()
        })?
        .to_string();
        if id != "1" {
            continue;
        }
        let name = call("CurrentName.button", false, || unsafe {
            element.CurrentName()
        })?
        .to_string();
        println!(
            "{}",
            json!({"event":"button_candidate","automation_id":id,"name":scrub(&name,root),"caption_match":button_caption(&name)})
        );
        if button_caption(&name) {
            ensure_target(uia, anchor, &element, window, pid)?;
            matches.push(element);
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "IDOK target count is {}, expected one named Open/Save button",
            matches.len()
        )
        .into());
    }
    matches.pop().ok_or_else(|| "IDOK target missing".into())
}

fn toolbar_name(
    uia: &IUIAutomation,
    window: HWND,
    pid: u32,
    root: &Path,
    phase: &str,
) -> AnyResult<String> {
    unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> BOOL {
        unsafe { &mut *(param.0 as *mut Vec<HWND>) }.push(window);
        BOOL(1)
    }
    let mut children = Vec::new();
    unsafe {
        let _ = EnumChildWindows(
            Some(window),
            Some(visit),
            LPARAM((&raw mut children) as isize),
        );
    }
    let mut names = Vec::new();
    for child in children {
        let n = native(child);
        if n["class"] != "ToolbarWindow32" || n["pid"] != pid || n["visible"] != true {
            continue;
        }
        let parent = unsafe { GetParent(child) }?;
        if native(parent)["class"] != "Breadcrumb Parent" {
            continue;
        }
        // The accepted trace has msctls_progress32 between Breadcrumb Parent
        // and Address Band Root. Validate bounded ancestry, not direct parentage.
        let mut cursor = parent;
        let mut chain = Vec::new();
        let mut saw_band = false;
        let mut reached_dialog = false;
        for _ in 0..16 {
            let state = native(cursor);
            if state["pid"] != pid || state["alive"] != true {
                break;
            }
            saw_band |= state["class"] == "Address Band Root";
            chain.push(state);
            if cursor == window {
                reached_dialog = true;
                break;
            }
            let Ok(next) = (unsafe { GetParent(cursor) }) else {
                break;
            };
            cursor = next;
        }
        if !saw_band || !reached_dialog {
            continue;
        }
        println!(
            "{}",
            json!({"event":"toolbar_native_ancestry", "phase":phase, "chain":chain})
        );
        let element = call("ElementFromHandle.toolbar", false, || unsafe {
            uia.ElementFromHandle(child)
        })?;
        let id = call("CurrentAutomationId.toolbar", false, || unsafe {
            element.CurrentAutomationId()
        })?
        .to_string();
        let ep = call("CurrentProcessId.toolbar", false, || unsafe {
            element.CurrentProcessId()
        })?;
        let actual = call("CurrentNativeWindowHandle.toolbar", false, || unsafe {
            element.CurrentNativeWindowHandle()
        })?;
        if id != "1001" || ep != i32::try_from(pid)? || actual != child {
            return Err("toolbar binding changed".into());
        }
        names.push(
            call("CurrentName.toolbar", false, || unsafe {
                element.CurrentName()
            })?
            .to_string(),
        );
    }
    println!(
        "{}",
        json!({"event":"external_folder_observation","phase":phase,"toolbar_names":names.iter().map(|n|scrub(n,root)).collect::<Vec<_>>(),"observed_at_ms":now_ms()})
    );
    if names.len() != 1 {
        return Err(format!("full-path toolbar count {}, expected one", names.len()).into());
    }
    names.pop().ok_or_else(|| "toolbar missing".into())
}

fn matches_folder(name: &str, path: &Path) -> bool {
    let name = name.trim_start_matches('\u{200e}').trim();
    let path_text = path.to_string_lossy();
    ["地址: ", "Address: "].iter().any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|s| {
            s.trim_end_matches('\\')
                .eq_ignore_ascii_case(path_text.trim_end_matches('\\'))
        })
    })
}

fn probe(window: HWND, root: &Path, pid: u32) -> AnyResult<()> {
    WORKER_START.with(|s| s.set(Some(Instant::now())));
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    println!(
        "{}",
        json!({"event":"worker_start","pid":std::process::id(),"foreground":native(unsafe{GetForegroundWindow()}),"observed_at_ms":now_ms()})
    );
    let result = (|| -> AnyResult<()> {
        let uia: IUIAutomation = call("CoCreateInstance.UIA", false, || unsafe {
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        })?;
        let anchor = call("ElementFromHandle.dialog", false, || unsafe {
            uia.ElementFromHandle(window)
        })?;
        let (_, _, original) = filename(
            &uia,
            &anchor,
            window,
            pid,
            root,
            Some("pending-prototype-name.csv"),
        )?;
        let _ = button(&uia, &anchor, window, pid, root)?;
        let before = toolbar_name(&uia, window, pid, root, "before")?;
        if !matches_folder(&before, &root.join("A")) {
            return Err("initial external toolbar does not match A".into());
        }
        let target = format!("{}\\", root.join("B").display());
        let (edit, value, _) = filename(&uia, &anchor, window, pid, root, Some(&original))?;
        ensure_target(&uia, &anchor, &edit, window, pid)?;
        call("Value.SetValue.target_directory", true, || unsafe {
            value.SetValue(&BSTR::from(target.as_str()))
        })?;
        let (_, _, assigned) = filename(&uia, &anchor, window, pid, root, Some(&target))?;
        println!(
            "{}",
            json!({"event":"assignment_verified","value":scrub(&assigned,root),"observed_at_ms":now_ms()})
        );
        let button = button(&uia, &anchor, window, pid, root)?;
        let invoke: IUIAutomationInvokePattern =
            call("GetCurrentPattern.Invoke", false, || unsafe {
                button.GetCurrentPatternAs(UIA_InvokePatternId)
            })?;
        ensure_target(&uia, &anchor, &button, window, pid)?;
        call("Invoke.IDOK", true, || unsafe { invoke.Invoke() })?;
        let started = Instant::now();
        let mut reached = false;
        while started.elapsed() < Duration::from_secs(3) {
            let name = toolbar_name(&uia, window, pid, root, "await_B")?;
            if matches_folder(&name, &root.join("B")) {
                reached = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        println!(
            "{}",
            json!({"event":"external_arrival_gate","matched_B":reached,"elapsed_ms":started.elapsed().as_millis(),"observed_at_ms":now_ms()})
        );
        if !reached {
            return Err(
                "external B arrival gate not established; no additional navigation action".into(),
            );
        }
        let (edit, value, _) = filename(&uia, &anchor, window, pid, root, None)?;
        ensure_target(&uia, &anchor, &edit, window, pid)?;
        call("Value.SetValue.restore_filename", true, || unsafe {
            value.SetValue(&BSTR::from(original.as_str()))
        })?;
        let _ = filename(&uia, &anchor, window, pid, root, Some(&original))?;
        let final_name = toolbar_name(&uia, window, pid, root, "after_restore")?;
        if !matches_folder(&final_name, &root.join("B")) {
            return Err("external folder changed during restoration".into());
        }
        println!(
            "{}",
            json!({"event":"worker_chain_complete","filename_restored":true,"external_B_after_restore":true,"observed_at_ms":now_ms()})
        );
        Ok(())
    })();
    println!(
        "{}",
        json!({"event":"worker_result","success":result.is_ok(),"stop_reason":result.as_ref().err().map(ToString::to_string),"uia_calls":CALLS.with(Cell::get),"foreground":native(unsafe{GetForegroundWindow()}),"observed_at_ms":now_ms()})
    );
    Ok(()) // A bounded unsupported result is evidence; inspect worker_result and native_verdict separately.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_open_save_commands() {
        assert!(button_caption("打开(&O)"));
        assert!(button_caption("Save (&S)"));
        assert!(!button_caption("Open with"));
        assert!(!button_caption("保存副本"));
        assert!(!button_caption("Cancel"));
    }
    #[test]
    fn arrival_name_requires_a_full_exact_folder() {
        assert!(matches_folder(
            "地址: C:\\fixture\\B",
            Path::new(r"C:\fixture\B")
        ));
        assert!(!matches_folder(
            "地址: C:\\fixture\\B2",
            Path::new(r"C:\fixture\B")
        ));
        assert!(!matches_folder("B", Path::new(r"C:\fixture\B")));
    }
}
