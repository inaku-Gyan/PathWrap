//! THROWAWAY: invoke the hidden address ComboBox's exact child Button once.
//! Run: cargo run --bin uia_address_combo_invoke_prototype
//! No setters, focus actions, follow-up actions or input simulation.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{E_ABORT, E_POINTER, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
    IUIAutomationInvokePattern, IUIAutomationLegacyIAccessiblePattern, IUIAutomationTreeWalker,
    IUIAutomationValuePattern, UIA_ButtonControlTypeId, UIA_ExpandCollapsePatternId,
    UIA_InvokePatternId, UIA_LegacyIAccessiblePatternId, UIA_ValuePatternId,
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
        return Err("usage: uia_address_combo_invoke_prototype".into());
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
            "PROTOTYPE — one address ComboBox Invoke ({mode})"
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
        "PathWarp-COMBO-INVOKE-PROTOTYPE-{}-{mode}",
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
            json!({"event": "probe_start", "mode": mode, "architecture": std::env::consts::ARCH, "host_pid": child.id(), "client_pid": std::process::id()})
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
        // A separate client process bounds every provider call, including Invoke.
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
                client.kill()?; // This command's UIA worker only; never retry Invoke.
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
                "note": "A timeout leaves the provider HRESULT unknown; there is no retry.",
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

fn next_element(
    result: windows::core::Result<IUIAutomationElement>,
) -> windows::core::Result<Option<IUIAutomationElement>> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(error) if error.code() == E_POINTER || error.code().is_ok() => Ok(None),
        Err(error) => Err(error),
    }
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
    if value.contains(":\\") || value.starts_with("\\\\") {
        value = "NON_FIXTURE_PATH".to_owned();
    }
    value
}

fn pattern_error<T>(result: &windows::core::Result<T>) -> Option<String> {
    result
        .as_ref()
        .err()
        .map(|error| format!("{:?}", error.code()))
}

fn inventory(
    uia: &IUIAutomation,
    window: HWND,
    root: &Path,
    phase: &str,
) -> AnyResult<Vec<(IUIAutomationElement, Value)>> {
    let root_element = unsafe { uia.ElementFromHandle(window)? };
    inventory_element(uia, root_element, root, phase)
}

fn inventory_element(
    uia: &IUIAutomation,
    root_element: IUIAutomationElement,
    root: &Path,
    phase: &str,
) -> AnyResult<Vec<(IUIAutomationElement, Value)>> {
    let walker: IUIAutomationTreeWalker = unsafe { uia.RawViewWalker()? };
    let mut pending = vec![(
        root_element,
        None::<usize>,
        0,
        phase.ends_with("_address_combo") || phase.contains("owned_popup"),
    )];
    let mut entries = Vec::new();
    while let Some((element, parent, depth, in_address)) = pending.pop() {
        let index = entries.len();
        if index >= 2_000 {
            return Err("raw tree exceeded the prototype's 2000-element bound".into());
        }
        let class = unsafe { element.CurrentClassName()? }.to_string();
        let address_scope = in_address || class == "Address Band Root";
        let name = unsafe { element.CurrentName()? }.to_string();
        let marker = matches!(name.as_str(), "marker-only-in-A" | "marker-only-in-B");
        let invoke = unsafe {
            element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
        };
        let value =
            unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) };
        let expand = unsafe {
            element.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(
                UIA_ExpandCollapsePatternId,
            )
        };
        let legacy = unsafe {
            element.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
                UIA_LegacyIAccessiblePatternId,
            )
        };
        let state = json!({
            "event": "element", "phase": phase, "index": index, "parent": parent, "depth": depth,
            "class": class, "address_scope": address_scope,
            "automation_id": unsafe { element.CurrentAutomationId()? }.to_string(),
            "name": if address_scope || marker || index == 0 { Some(scrub(&name, root)) } else { None },
            "control_type": unsafe { element.CurrentControlType()? }.0,
            "framework": unsafe { element.CurrentFrameworkId()? }.to_string(),
            "provider": unsafe { element.CurrentProviderDescription()? }.to_string(),
            "pid": unsafe { element.CurrentProcessId()? },
            "hwnd": unsafe { element.CurrentNativeWindowHandle()? }.0 as usize,
            "enabled": unsafe { element.CurrentIsEnabled()? }.as_bool(),
            "offscreen": unsafe { element.CurrentIsOffscreen()? }.as_bool(),
            "focusable": unsafe { element.CurrentIsKeyboardFocusable()? }.as_bool(),
            "patterns": {
                "invoke": {"available": invoke.is_ok(), "binding_error_hresult": pattern_error(&invoke)},
                "value": {"available": value.is_ok(), "binding_error_hresult": pattern_error(&value)},
                "expand_collapse": {"available": expand.is_ok(), "binding_error_hresult": pattern_error(&expand)},
                "legacy": {"available": legacy.is_ok(), "binding_error_hresult": pattern_error(&legacy)},
            },
            "value_readonly": value.as_ref().ok().and_then(|pattern| unsafe { pattern.CurrentIsReadOnly() }.ok()).map(|value| value.as_bool()),
            "value": if address_scope { value.as_ref().ok().and_then(|pattern| unsafe { pattern.CurrentValue() }.ok()).map(|value| scrub(&value.to_string(), root)) } else { None },
            "legacy_role": if address_scope { legacy.as_ref().ok().and_then(|pattern| unsafe { pattern.CurrentRole() }.ok()) } else { None },
            "legacy_default_action": if address_scope { legacy.as_ref().ok().and_then(|pattern| unsafe { pattern.CurrentDefaultAction() }.ok()).map(|value| scrub(&value.to_string(), root)) } else { None },
        });
        println!("{state}");
        let mut children = Vec::new();
        let mut child = next_element(unsafe { walker.GetFirstChildElement(&element) })?;
        while let Some(current) = child {
            child = next_element(unsafe { walker.GetNextSiblingElement(&current) })?;
            children.push(current);
        }
        for child in children.into_iter().rev() {
            pending.push((child, Some(index), depth + 1, address_scope));
        }
        entries.push((element, state));
    }
    Ok(entries)
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

fn probe(window: HWND, root: &Path, host_pid: u32) -> AnyResult<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let uia: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let entries = inventory(&uia, window, root, "initial")?;
    let bands: Vec<_> = entries
        .iter()
        .filter(|(_, state)| state["class"] == "Address Band Root")
        .collect();
    if bands.len() != 1 {
        return stop("address_band_not_unambiguous", bands.len());
    }
    let band_window = unsafe { bands[0].0.CurrentNativeWindowHandle()? };
    if bands[0].1["pid"] != host_pid || !unsafe { IsChild(window, band_window).as_bool() } {
        return stop("address_band_identity_mismatch", 0);
    }
    let mut combos = Vec::new();
    for native in address_windows(band_window) {
        let state = native_info(native);
        println!(
            "{}",
            json!({"event": "address_hwnd", "phase": "before", "state": state})
        );
        if state["class"] == "ComboBox" && state["pid"] == host_pid {
            combos.push(native);
        }
    }
    if combos.len() != 1 {
        return stop("native_address_combo_not_unambiguous", combos.len());
    }
    let combo_window = combos[0];
    let combo = unsafe { uia.ElementFromHandle(combo_window)? };
    if unsafe { combo.CurrentNativeWindowHandle()? } != combo_window
        || unsafe { combo.CurrentProcessId()? } != i32::try_from(host_pid)?
        || !unsafe { IsChild(band_window, combo_window).as_bool() }
    {
        return stop("native_address_combo_binding_mismatch", 0);
    }
    let children = inventory_element(&uia, combo.clone(), root, "before_address_combo")?;
    let targets: Vec<_> = children
        .iter()
        .filter(|(_, state)| {
            // These metadata came from this zh-CN fixture, not universal IDs.
            state["parent"] == 0
                && state["control_type"] == UIA_ButtonControlTypeId.0
                && state["pid"] == host_pid
                && state["name"] == "地址"
                && state["legacy_role"] == 43
                && state["legacy_default_action"] == "打开"
                && state["enabled"] == true
                && state["patterns"]["invoke"]["available"] == true
        })
        .collect();
    if targets.len() != 1 {
        return stop("exact_address_combo_child_not_unambiguous", targets.len());
    }
    let target = targets[0];
    let walker = unsafe { uia.RawViewWalker()? };
    let parent = unsafe { walker.GetParentElement(&target.0)? };
    let parent_matches = unsafe { uia.CompareElements(&parent, &combo)?.as_bool() };
    if !parent_matches {
        return stop("exact_child_parent_mismatch", 0);
    }
    let mut ancestors = Vec::new();
    let mut ancestor = Some(combo_window);
    while let Some(current) = ancestor {
        ancestors.push(native_info(current));
        if current == window || ancestors.len() == 16 {
            break;
        }
        ancestor = unsafe { GetParent(current) }.ok();
    }
    println!(
        "{}",
        json!({
            "event": "target_identity", "target": target.1, "combo": children[0].1,
            "band": bands[0].1, "native_ancestors": ancestors, "uia_parent_matches": parent_matches,
            "native_combo_visible": unsafe { IsWindowVisible(combo_window).as_bool() },
            "uia_child_offscreen": target.1["offscreen"],
        })
    );
    observe_focus(&uia, root, host_pid, "before")?;
    owned_popups(&uia, window, root, host_pid, "before")?;
    let invoke = unsafe {
        target
            .0
            .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?
    };
    println!(
        "{}",
        json!({
            "event": "invoke_start", "invoke_calls_started": 1, "observed_at_ms": now_ms(),
            "target": target.1,
        })
    );
    std::io::stdout().flush()?;
    let started = Instant::now();
    let result = unsafe { invoke.Invoke() }; // The ONLY provider action in this binary.
    let elapsed = started.elapsed();
    println!(
        "{}",
        json!({
            "event": "invoke_result", "invoke_calls": 1, "success": result.is_ok(),
            "hresult": result.as_ref().err().map_or("HRESULT(0x00000000)".to_owned(), |error| format!("{:?}", error.code())),
            "elapsed_us": elapsed.as_micros(), "observed_at_ms": now_ms(),
        })
    );
    let observation_start = Instant::now();
    println!(
        "{}",
        json!({"event": "observation_start", "duration_ms": 3000, "observed_at_ms": now_ms()})
    );
    for offset in [0_u64, 1000, 2000, 2900] {
        std::thread::sleep(
            Duration::from_millis(offset).saturating_sub(observation_start.elapsed()),
        );
        let phase = format!("after_{offset}ms");
        observe_focus(&uia, root, host_pid, &phase)?;
        inventory(&uia, window, root, &phase)?;
        let fresh_combo = unsafe { uia.ElementFromHandle(combo_window)? };
        inventory_element(&uia, fresh_combo, root, &format!("{phase}_address_combo"))?;
        for native in address_windows(band_window) {
            println!(
                "{}",
                json!({"event": "address_hwnd", "phase": phase, "state": native_info(native)})
            );
        }
        owned_popups(&uia, window, root, host_pid, &phase)?;
    }
    std::thread::sleep(Duration::from_secs(3).saturating_sub(observation_start.elapsed()));
    println!(
        "{}",
        json!({
            "event": "probe_done", "invoke_calls": 1, "set_focus_calls": 0, "set_value_calls": 0,
            "followup_actions": 0, "observation_elapsed_ms": observation_start.elapsed().as_millis(),
            "observed_at_ms": now_ms(), "dialog_alive": unsafe { IsWindow(Some(window)).as_bool() },
        })
    );
    Ok(())
}

fn stop(reason: &str, candidates: usize) -> AnyResult<()> {
    println!(
        "{}",
        json!({"event": "probe_stopped", "reason": reason, "candidates": candidates, "invoke_calls": 0})
    );
    Ok(())
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

fn observe_focus(uia: &IUIAutomation, root: &Path, host_pid: u32, phase: &str) -> AnyResult<()> {
    let focused = unsafe { uia.GetFocusedElement() };
    let focus = if let Ok(element) = focused {
        let pid = unsafe { element.CurrentProcessId()? };
        json!({
            "pid": pid, "hwnd": unsafe { element.CurrentNativeWindowHandle()? }.0 as usize,
            "class": unsafe { element.CurrentClassName()? }.to_string(),
            "control_type": unsafe { element.CurrentControlType()? }.0,
            "name": if pid == i32::try_from(host_pid)? { Some(scrub(&unsafe { element.CurrentName()? }.to_string(), root)) } else { None },
        })
    } else {
        json!({"unavailable": true, "hresult": pattern_error(&focused)})
    };
    println!(
        "{}",
        json!({
            "event": "focus_foreground", "phase": phase, "observed_at_ms": now_ms(),
            "focused": focus, "foreground": native_info(unsafe { GetForegroundWindow() }),
        })
    );
    Ok(())
}

fn owned_popups(
    uia: &IUIAutomation,
    dialog: HWND,
    root: &Path,
    host_pid: u32,
    phase: &str,
) -> AnyResult<()> {
    let walker = unsafe { uia.RawViewWalker()? };
    let desktop = unsafe { uia.GetRootElement()? };
    let mut child = next_element(unsafe { walker.GetFirstChildElement(&desktop) })?;
    let mut captured = Vec::new();
    let mut uia_roots = 0;
    while let Some(element) = child {
        child = next_element(unsafe { walker.GetNextSiblingElement(&element) })?;
        // Read only process identity on unrelated desktop children, never their trees/names.
        if unsafe { element.CurrentProcessId()? } == i32::try_from(host_pid)? {
            let hwnd = unsafe { element.CurrentNativeWindowHandle()? };
            if hwnd != dialog {
                inventory_element(
                    uia,
                    element,
                    root,
                    &format!("{phase}_owned_popup_uia_{uia_roots}"),
                )?;
                captured.push(hwnd);
                uia_roots += 1;
            }
        }
    }
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
        unsafe { &mut *(data.0 as *mut Vec<HWND>) }.push(window);
        BOOL(1)
    }
    let mut windows = Vec::new();
    unsafe { EnumWindows(Some(collect), LPARAM((&raw mut windows) as isize))? };
    let mut native_roots = 0;
    let mut native_visible_roots = 0;
    for window in windows {
        let state = native_info(window);
        if state["pid"] == host_pid && window != dialog {
            if state["visible"] == true {
                native_visible_roots += 1;
            }
            println!(
                "{}",
                json!({"event": "owned_native_window", "phase": phase, "state": state})
            );
            if state["visible"] == true && !captured.contains(&window) {
                let element = unsafe { uia.ElementFromHandle(window)? };
                inventory_element(
                    uia,
                    element,
                    root,
                    &format!("{phase}_owned_popup_native_{native_roots}"),
                )?;
            }
            native_roots += 1;
        }
    }
    println!(
        "{}",
        json!({
            "event": "owned_popup_summary", "phase": phase,
            "uia_desktop_roots_excluding_dialog": uia_roots, "native_roots_excluding_dialog": native_roots,
            "native_visible_roots_excluding_dialog": native_visible_roots,
            "observed_at_ms": now_ms(),
        })
    );
    Ok(())
}
