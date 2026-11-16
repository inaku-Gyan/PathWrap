//! THROWAWAY PROTOTYPE: can an external UIA ObjectModel expose IFileDialog?
//! Run: cargo run --bin native_dialog_navigation_prototype -- open
//! Repeat with `save`. This is a real API capability probe, not a state-model demo.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::collections::BTreeMap;
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
use windows::Win32::System::Variant::{VariantClear, VariantToBoolean};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationObjectModelPattern,
    IUIAutomationTreeWalker, UIA_IsObjectModelPatternAvailablePropertyId, UIA_ObjectModelPatternId,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FOS_DONTADDTORECENT, FOS_FORCEFILESYSTEM, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileDialogEvents, IFileDialogEvents_Impl, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW,
    WM_CLOSE,
};
use windows::core::{BOOL, HSTRING, Interface, PWSTR, Ref, implement, w};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--host") {
        return host(&args[2], Path::new(&args[3]));
    }
    let mode = args.get(1).map_or("open", String::as_str);
    if !matches!(mode, "open" | "save") {
        return Err("usage: native_dialog_navigation_prototype [open|save]".into());
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
            "PROTOTYPE — native interface ({mode})"
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
    std::thread::spawn(|| {
        for line in std::io::stdin().lock().lines() {
            if line.is_ok_and(|line| line == "close") {
                close_fixture();
                break;
            }
        }
    });
    let result = unsafe { dialog.Show(None) };
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
    let root =
        std::env::temp_dir().join(format!("PathWarp-PROTOTYPE-{}-{mode}", std::process::id()));
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
        probe(window, &root, &receive)?;
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

fn probe(window: HWND, root: &Path, events: &mpsc::Receiver<Value>) -> AnyResult<()> {
    let uia: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("Create UIA: {error}"))?;
    let walker: IUIAutomationTreeWalker =
        unsafe { uia.RawViewWalker() }.map_err(|error| format!("Get raw-view walker: {error}"))?;
    let root_element = unsafe { uia.ElementFromHandle(window) }
        .map_err(|error| format!("Get target UIA root: {error}"))?;
    let mut pending = vec![root_element];
    let mut nodes = 0;
    let mut available = 0;
    let mut pattern_failures = BTreeMap::<String, usize>::new();
    while let Some(element) = pending.pop() {
        nodes += 1;
        if nodes > 2_000 {
            return Err("raw tree exceeded the prototype's 2000-element bound".into());
        }
        let mut property =
            unsafe { element.GetCurrentPropertyValue(UIA_IsObjectModelPatternAvailablePropertyId) }
                .map_err(|error| format!("Read availability at element {nodes}: {error}"))?;
        let reported = unsafe { VariantToBoolean(&property) };
        unsafe { VariantClear(&mut property)? };
        let reported = reported
            .map_err(|error| format!("Convert availability at element {nodes}: {error}"))?
            .as_bool();
        available += usize::from(reported);
        let pattern = unsafe {
            element.GetCurrentPatternAs::<IUIAutomationObjectModelPattern>(UIA_ObjectModelPatternId)
        };
        println!(
            "{}",
            json!({
                "event": "element", "index": nodes,
                "class": unsafe { element.CurrentClassName()? }.to_string(),
                "automation_id": unsafe { element.CurrentAutomationId()? }.to_string(),
                "control_type": unsafe { element.CurrentControlType()? }.0,
                "object_model_available": reported,
                "pattern_hresult": pattern.as_ref().err().map(|error| format!("{:?}", error.code())),
            })
        );
        match pattern {
            Ok(pattern) => {
                let object = unsafe { pattern.GetUnderlyingObjectModel()? };
                let dialog = object.cast::<IFileDialog>()?;
                if unsafe { dialog.cast::<IOleWindow>()?.GetWindow()? } != window {
                    return Err("native object does not identify the original target dialog".into());
                }
                return navigate(&dialog, window, root, events);
            }
            Err(error) => {
                *pattern_failures
                    .entry(format!("{:?}", error.code()))
                    .or_default() += 1;
                if reported {
                    return Err(format!(
                        "ObjectModel reported available but retrieval failed: {error}"
                    )
                    .into());
                }
            }
        }
        let mut child = next_element(unsafe { walker.GetFirstChildElement(&element) })?;
        while let Some(current) = child {
            child = next_element(unsafe { walker.GetNextSiblingElement(&current) })?;
            pending.push(current);
        }
    }
    println!(
        "{}",
        json!({
            "event": "verdict", "result": "native_discovery_unavailable_in_this_sample",
            "raw_elements": nodes, "object_model_available": available,
            "pattern_failures": pattern_failures, "set_folder_calls": 0,
            "dialog_still_open": unsafe { IsWindow(Some(window)).as_bool() },
        })
    );
    Ok(())
}

fn navigate(
    dialog: &IFileDialog,
    window: HWND,
    root: &Path,
    events: &mpsc::Receiver<Value>,
) -> AnyResult<()> {
    let before = snapshot(dialog, root)?;
    println!("{}", json!({"event": "native_before", "state": before}));
    let destination = root.join("B");
    let target: IShellItem = unsafe {
        SHCreateItemFromParsingName(&HSTRING::from(destination.to_string_lossy().as_ref()), None)?
    };
    unsafe { dialog.SetFolder(&target)? };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if folder(dialog)?.eq_ignore_ascii_case(destination.to_string_lossy().as_ref()) {
            break;
        }
        if Instant::now() >= deadline {
            return Err("SetFolder returned, but arrival could not be confirmed".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let after = snapshot(dialog, root)?;
    let mut host_confirmed = false;
    while Instant::now() < deadline {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(100))
            && event["folder"] == "PROTOTYPE_ROOT\\B"
            && event["filename"] == before["filename"]
            && event["file_type"] == before["file_type"]
        {
            host_confirmed = true;
            break;
        }
    }
    println!(
        "{}",
        json!({
            "event": "verdict", "result": "native_request_observed",
            "set_folder_calls": 1, "before": before, "after": after,
            "independent_host_confirmed": host_confirmed,
            "dialog_still_open": unsafe { IsWindow(Some(window)).as_bool() },
        })
    );
    Ok(())
}
