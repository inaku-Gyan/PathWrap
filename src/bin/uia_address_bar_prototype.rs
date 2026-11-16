//! THROWAWAY PROTOTYPE: can address-bar UIA navigate without simulated input?
//! Run: cargo run --bin uia_address_bar_prototype -- open
//! Repeat with `save`. This is a real API capability probe, not a state-model demo.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_ABORT, E_POINTER, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationElement3,
    IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern,
    IUIAutomationLegacyIAccessiblePattern, IUIAutomationTreeWalker, IUIAutomationValuePattern,
    UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_LegacyIAccessiblePatternId,
    UIA_ValuePatternId,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FOS_DONTADDTORECENT, FOS_FORCEFILESYSTEM, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileDialogEvents, IFileDialogEvents_Impl, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, KillTimer,
    PostMessageW, SetTimer, WM_CLOSE,
};
use windows::core::{BOOL, HSTRING, Interface, PWSTR, Ref, implement, w};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

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
    let mode = args.get(1).map_or("open", String::as_str);
    if !matches!(mode, "open" | "save") {
        return Err("usage: uia_address_bar_prototype [open|save]".into());
    }
    run(mode)
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
            "PROTOTYPE — address-bar UIA ({mode})"
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

fn run(mode: &str) -> AnyResult<()> {
    let root = std::env::temp_dir().join(format!(
        "PathWarp-UIA-PROTOTYPE-{}-{mode}",
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
    std::thread::spawn(move || {
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
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
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
        probe(window, &root, &receive, &before)?;
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
        phase == "address_context_menu",
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

fn popup_roots(
    uia: &IUIAutomation,
    pid: u32,
    dialog: HWND,
) -> AnyResult<Vec<IUIAutomationElement>> {
    let walker = unsafe { uia.RawViewWalker()? };
    let desktop = unsafe { uia.GetRootElement()? };
    let mut child = next_element(unsafe { walker.GetFirstChildElement(&desktop) })?;
    let mut roots = Vec::new();
    while let Some(current) = child {
        child = next_element(unsafe { walker.GetNextSiblingElement(&current) })?;
        if unsafe { current.CurrentProcessId()? } == i32::try_from(pid)?
            && unsafe { current.CurrentNativeWindowHandle()? } != dialog
        {
            roots.push(current);
        }
    }
    Ok(roots)
}

fn show_context_menu(window: HWND) -> mpsc::Receiver<Value> {
    let (send, receive) = mpsc::channel();
    let raw = window.0 as usize;
    std::thread::spawn(move || {
        let started = Instant::now();
        let result = (|| -> windows::core::Result<()> {
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
            let uia: IUIAutomation =
                unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
            let element = unsafe { uia.ElementFromHandle(HWND(raw as *mut _))? };
            unsafe { element.cast::<IUIAutomationElement3>()?.ShowContextMenu() }
        })();
        let _ = send.send(json!({
            "event": "action_result", "action": "ShowContextMenu", "success": result.is_ok(),
            "hresult": result.as_ref().err().map_or("HRESULT(0x00000000)".to_owned(), |error| format!("{:?}", error.code())),
            "elapsed_ms": started.elapsed().as_millis(),
        }));
    });
    receive
}

fn probe(window: HWND, root: &Path, _: &mpsc::Receiver<Value>, before: &Value) -> AnyResult<()> {
    let uia: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let entries = inventory(&uia, window, root, "initial")?;
    let toolbars: Vec<_> = entries
        .iter()
        .filter(|(_, state)| {
            state["address_scope"] == true
                && state["class"] == "ToolbarWindow32"
                && state["parent"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok())
                    .is_some_and(|index| entries[index].1["class"] == "Breadcrumb Parent")
        })
        .collect();
    if toolbars.len() != 1 {
        return Err("fixture address toolbar is not unambiguously identified".into());
    }
    let target = unsafe { toolbars[0].0.CurrentNativeWindowHandle()? };
    println!(
        "{}",
        json!({"event": "action_start", "action": "ShowContextMenu", "target": toolbars[0].1})
    );
    let action = show_context_menu(target);
    let mut host_pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut host_pid)) };
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut action_result = None;
    let menus = loop {
        if let Ok(result) = action.try_recv() {
            println!("{result}");
            action_result = Some(result);
        }
        let menus = popup_roots(&uia, host_pid, window)?;
        if !menus.is_empty() {
            break menus;
        }
        if Instant::now() >= deadline
            || action_result
                .as_ref()
                .is_some_and(|result| result["success"] == false)
        {
            inventory(&uia, window, root, "after_context_menu")?;
            println!(
                "{}",
                json!({
                    "event": "verdict", "result": "context_menu_not_observed", "action": action_result,
                    "before": before, "navigation_actions": 0,
                    "dialog_still_open": unsafe { IsWindow(Some(window)).as_bool() },
                })
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    for menu in menus {
        inventory_element(&uia, menu, root, "address_context_menu")?;
    }
    println!(
        "{}",
        json!({
        "event": "verdict", "result": "context_menu_inventory_complete",
            "elements": entries.len(), "before": before, "navigation_actions": 0,
            "dialog_still_open": unsafe { IsWindow(Some(window)).as_bool() },
        })
    );
    Ok(())
}
