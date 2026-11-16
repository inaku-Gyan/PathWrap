//! THROWAWAY PROTOTYPE: can external MSAA native-object access expose IFileDialog?
//! Run: cargo run --bin msaa_native_dialog_prototype -- open
//! Repeat with `save`. This is a real API capability probe, not a state-model demo.
#![allow(clippy::print_stdout)]

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_ABORT, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, IDispatch,
};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetCurrentProcessId};
use windows::Win32::UI::Accessibility::IAccessible;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FOS_DONTADDTORECENT, FOS_FORCEFILESYSTEM, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileDialogEvents, IFileDialogEvents_Impl, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetDlgCtrlID, GetWindowThreadProcessId, IsWindow,
    IsWindowVisible, OBJID_CLIENT, OBJID_NATIVEOM, PostMessageW, WM_CLOSE,
};
use windows::core::{BOOL, GUID, HRESULT, HSTRING, IUnknown, Interface, PWSTR, Ref, implement, w};

// The generated Result<()> binding erases successful HRESULT values. Keep the raw
// documented ABI here so the evidence records the exact result and nullness.
#[link(name = "oleacc")]
unsafe extern "system" {
    #[link_name = "AccessibleObjectFromWindow"]
    fn accessible_object_from_window_raw(
        window: HWND,
        object_id: u32,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT;
}

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--host") {
        return host(&args[2], Path::new(&args[3]));
    }
    let mode = args.get(1).map_or("open", String::as_str);
    if !matches!(mode, "open" | "save") {
        return Err("usage: msaa_native_dialog_prototype [open|save]".into());
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
            "PROTOTYPE — MSAA native access ({mode})"
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
    let root = std::env::temp_dir().join(format!(
        "PathWarp-MSAA-PROTOTYPE-{}-{mode}",
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

fn hresult(value: HRESULT) -> String {
    format!("0x{:08X}", u32::from_ne_bytes(value.0.to_ne_bytes()))
}

fn request<T: Interface>(
    window: HWND,
    index: usize,
    object_id: i32,
    iid_name: &str,
) -> (HRESULT, Option<T>) {
    let mut raw = std::ptr::null_mut();
    let result = unsafe {
        accessible_object_from_window_raw(
            window,
            u32::from_ne_bytes(object_id.to_ne_bytes()),
            &T::IID,
            &mut raw,
        )
    };
    println!(
        "{}",
        json!({
            "event": "access_request", "window_index": index, "object_id": object_id,
            "object_id_hex": format!("0x{:08X}", u32::from_ne_bytes(object_id.to_ne_bytes())),
            "iid_name": iid_name, "iid": format!("{:?}", T::IID),
            "hresult": hresult(result), "null_returned": raw.is_null(),
        })
    );
    let object = if result.is_ok() && !raw.is_null() {
        Some(unsafe { T::from_raw(raw) })
    } else {
        None
    };
    (result, object)
}

fn query_dialog<T: Interface>(
    object: Option<T>,
    index: usize,
    source: &str,
) -> Option<IFileDialog> {
    let object = object?;
    let result = object.cast::<IFileDialog>();
    println!(
        "{}",
        json!({
            "event": "query_interface", "window_index": index, "source": source,
            "iid_name": "IFileDialog", "iid": format!("{:?}", IFileDialog::IID),
            "hresult": hresult(result.as_ref().err().map_or(HRESULT(0), |error| error.code())),
            "interface_obtained": result.is_ok(),
        })
    );
    result.ok()
}

fn probe(window: HWND, root: &Path, events: &mpsc::Receiver<Value>) -> AnyResult<()> {
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
        unsafe { &mut *(data.0 as *mut Vec<HWND>) }.push(window);
        BOOL(1)
    }
    let mut windows = vec![window];
    let _ = unsafe {
        EnumChildWindows(
            Some(window),
            Some(collect),
            LPARAM((&raw mut windows) as isize),
        )
    };
    if windows.len() > 512 {
        return Err("fixture exceeded the prototype's 512-window bound".into());
    }
    let mut host_pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut host_pid)) };
    let mut counts = BTreeMap::<String, usize>::new();
    let mut native_objects = 0;
    let mut tested_windows = 0;
    // Diagnostic only: ordinary accessibility is not a native-navigation route.
    let (_, ordinary_accessible) = request::<IAccessible>(window, 0, OBJID_CLIENT.0, "IAccessible");
    println!(
        "{}",
        json!({"event": "ordinary_accessibility_control", "interface_obtained": ordinary_accessible.is_some()})
    );
    for (index, provider) in windows.iter().copied().enumerate() {
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(provider, Some(&mut pid)) };
        if pid != host_pid {
            return Err("enumerated window is no longer owned by the fixture".into());
        }
        let mut class = [0_u16; 256];
        let length = usize::try_from(unsafe { GetClassNameW(provider, &mut class) })?;
        tested_windows += 1;
        println!(
            "{}",
            json!({
                "event": "provider_window", "index": index, "hwnd": provider.0 as usize,
                "pid": pid, "thread_id": thread, "class": String::from_utf16_lossy(&class[..length]),
                "control_id": unsafe { GetDlgCtrlID(provider) },
                "visible": unsafe { IsWindowVisible(provider).as_bool() },
            })
        );
        let (unknown_hr, unknown) =
            request::<IUnknown>(provider, index, OBJID_NATIVEOM.0, "IUnknown");
        let (dispatch_hr, dispatch) =
            request::<IDispatch>(provider, index, OBJID_NATIVEOM.0, "IDispatch");
        let (dialog_hr, direct) =
            request::<IFileDialog>(provider, index, OBJID_NATIVEOM.0, "IFileDialog");
        native_objects += usize::from(unknown.is_some())
            + usize::from(dispatch.is_some())
            + usize::from(direct.is_some());
        for result in [unknown_hr, dispatch_hr, dialog_hr] {
            *counts.entry(hresult(result)).or_default() += 1;
        }
        let from_unknown = query_dialog(unknown, index, "OBJID_NATIVEOM/IUnknown");
        let from_dispatch = query_dialog(dispatch, index, "OBJID_NATIVEOM/IDispatch");
        if let Some(dialog) = direct.or(from_unknown).or(from_dispatch) {
            if unsafe { dialog.cast::<IOleWindow>()?.GetWindow()? } != window {
                return Err("native interface does not identify the original target dialog".into());
            }
            if !folder(&dialog)?.eq_ignore_ascii_case(root.join("A").to_string_lossy().as_ref()) {
                return Err(
                    "native interface does not read the fixture's actual initial folder".into(),
                );
            }
            return navigate(&dialog, window, root, events);
        }
    }
    println!(
        "{}",
        json!({
            "event": "verdict", "result": "native_discovery_unavailable_in_this_sample",
            "provider_windows": tested_windows, "native_interface_returns": native_objects,
            "native_request_hresults": counts, "set_folder_calls": 0,
            "ordinary_accessible_obtained": ordinary_accessible.is_some(),
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
