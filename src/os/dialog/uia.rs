//! Sole UIA strategy. No input simulation, alternate getter or native navigation.
use crate::core::navigation::{
    self, Dialog, DialogIdentity, Failure, ProviderResult, Report, Request, Snapshot, WORKER_BUDGET,
};
use std::cell::Cell;
use std::time::Instant;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationValuePattern, TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_ComboBoxControlTypeId, UIA_ControlTypePropertyId, UIA_EditControlTypeId,
    UIA_InvokePatternId, UIA_SplitButtonControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetForegroundWindow, GetParent, GetWindowThreadProcessId,
    IsChild, IsWindow, IsWindowVisible,
};
use windows::core::{BOOL, BSTR};
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
pub(super) fn run(request: Request, progress: impl FnMut(&Report)) -> Report {
    let started = Instant::now();
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        let mut report = Report::new(request);
        report.reason = format!("COM initialization failed: {error}");
        return report;
    }
    let _com = Com;
    match Adapter::new(request.clone(), started) {
        Ok(mut adapter) => navigation::execute(&mut adapter, request, progress),
        Err(error) => {
            let mut report = Report::new(request);
            if error.interrupted {
                report.outcome = navigation::Outcome::Interrupted;
            }
            report.reason = error.reason;
            report
        }
    }
}
pub fn identity(handle: isize) -> Option<DialogIdentity> {
    let window = HWND(handle as *mut _);
    if handle == 0 || !unsafe { IsWindow(Some(window)).as_bool() } {
        return None;
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    let result =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    unsafe {
        let _ = CloseHandle(process);
    }
    result.ok()?;
    Some(DialogIdentity {
        hwnd: handle,
        pid,
        process_created: (u64::from(created.dwHighDateTime) << 32)
            | u64::from(created.dwLowDateTime),
    })
}
struct Adapter {
    request: Request,
    uia: IUIAutomation,
    anchor: IUIAutomationElement,
    started: Instant,
    calls: Cell<usize>,
}
impl Adapter {
    fn new(request: Request, started: Instant) -> ProviderResult<Self> {
        if identity(request.dialog.hwnd) != Some(request.dialog) {
            return Err(Failure::interrupted(
                "original dialog/process identity is unavailable",
            ));
        }
        let uia: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| Failure::provider(error.to_string()))?;
        let anchor = unsafe { uia.ElementFromHandle(HWND(request.dialog.hwnd as *mut _)) }
            .map_err(|error| Failure::provider(error.to_string()))?;
        let adapter = Self {
            request,
            uia,
            anchor,
            started,
            calls: Cell::new(0),
        };
        adapter.check()?;
        Ok(adapter)
    }
    fn window(&self) -> HWND {
        HWND(self.request.dialog.hwnd as *mut _)
    }
    fn call<T>(&self, f: impl FnOnce() -> windows::core::Result<T>) -> ProviderResult<T> {
        let count = self.calls.get() + 1;
        self.calls.set(count);
        if count > 2048 || self.started.elapsed() >= WORKER_BUDGET {
            return Err(Failure::provider("UIA call/worker budget exhausted"));
        }
        f().map_err(|error| Failure::provider(error.to_string()))
    }
    fn check(&self) -> ProviderResult<()> {
        let window = self.window();
        let foreground = unsafe { GetForegroundWindow() }.0 as isize;
        if identity(self.request.dialog.hwnd) != Some(self.request.dialog)
            || !unsafe { IsWindowVisible(window).as_bool() && IsWindowEnabled(window).as_bool() }
            || (foreground != self.request.dialog.hwnd && foreground != self.request.overlay_hwnd)
        {
            return Err(Failure::interrupted(
                "original dialog or foreground interaction is no longer active",
            ));
        }
        let fresh = self.call(|| unsafe { self.uia.ElementFromHandle(window) })?;
        if !self
            .call(|| unsafe { self.uia.CompareElements(&self.anchor, &fresh) })?
            .as_bool()
        {
            return Err(Failure::interrupted("original UIA root was replaced"));
        }
        Ok(())
    }
    fn guarded(&self, element: &IUIAutomationElement) -> ProviderResult<()> {
        self.check()?;
        let pid = self.call(|| unsafe { element.CurrentProcessId() })?;
        let handle = self.call(|| unsafe { element.CurrentNativeWindowHandle() })?;
        if u32::try_from(pid).ok() != Some(self.request.dialog.pid)
            || !self
                .call(|| unsafe { element.CurrentIsEnabled() })?
                .as_bool()
            || self
                .call(|| unsafe { element.CurrentIsOffscreen() })?
                .as_bool()
        {
            return Err(Failure::interrupted("control ownership/visibility changed"));
        }
        if !handle.is_invalid() {
            let mut native_pid = 0;
            unsafe {
                GetWindowThreadProcessId(handle, Some(&mut native_pid));
            }
            if native_pid != self.request.dialog.pid
                || !unsafe { IsChild(self.window(), handle).as_bool() }
                || !unsafe {
                    IsWindowVisible(handle).as_bool() && IsWindowEnabled(handle).as_bool()
                }
            {
                return Err(Failure::interrupted(
                    "control native ownership/ancestry changed",
                ));
            }
        }
        let walker = self.call(|| unsafe { self.uia.RawViewWalker() })?;
        let mut cursor = element.clone();
        for _ in 0..16 {
            if self
                .call(|| unsafe { self.uia.CompareElements(&cursor, &self.anchor) })?
                .as_bool()
            {
                return Ok(());
            }
            cursor = self.call(|| unsafe { walker.GetParentElement(&cursor) })?;
        }
        Err(Failure::interrupted(
            "control is not under the original UIA root",
        ))
    }
    fn descendants(&self, types: &[i32]) -> ProviderResult<Vec<IUIAutomationElement>> {
        self.check()?;
        let first = self.call(|| unsafe {
            self.uia
                .CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(types[0]))
        })?;
        let condition = if types.len() == 2 {
            let second = self.call(|| unsafe {
                self.uia
                    .CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(types[1]))
            })?;
            self.call(|| unsafe { self.uia.CreateOrCondition(&first, &second) })?
        } else {
            first
        };
        let found =
            self.call(|| unsafe { self.anchor.FindAll(TreeScope_Descendants, &condition) })?;
        let count = self.call(|| unsafe { found.Length() })?;
        if !(0..=64).contains(&count) {
            return Err(Failure::provider("typed lookup exceeds 64-element bound"));
        }
        (0..count)
            .map(|i| self.call(|| unsafe { found.GetElement(i) }))
            .collect()
    }
    fn filename_control(
        &self,
    ) -> ProviderResult<(IUIAutomationElement, IUIAutomationValuePattern)> {
        let mut matches = Vec::new();
        for edit in self.descendants(&[UIA_EditControlTypeId.0])? {
            let id = self
                .call(|| unsafe { edit.CurrentAutomationId() })?
                .to_string();
            if !matches!(id.as_str(), "1148" | "1001" | "FileNameControlHost") {
                continue;
            }
            let name = self.call(|| unsafe { edit.CurrentName() })?.to_string();
            if !name.contains("文件名") && !name.to_lowercase().contains("file name") {
                continue;
            }
            self.guarded(&edit)?;
            let value: IUIAutomationValuePattern =
                self.call(|| unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId) })?;
            if !self
                .call(|| unsafe { value.CurrentIsReadOnly() })?
                .as_bool()
            {
                matches.push((edit, value));
            }
        }
        if matches.len() != 1 {
            return Err(Failure::provider(
                "filename control is missing, ambiguous or read-only",
            ));
        }
        matches
            .pop()
            .ok_or_else(|| Failure::provider("filename missing"))
    }
    fn command(&self) -> ProviderResult<(IUIAutomationElement, IUIAutomationInvokePattern)> {
        let mut matches = Vec::new();
        for button in
            self.descendants(&[UIA_ButtonControlTypeId.0, UIA_SplitButtonControlTypeId.0])?
        {
            if self.call(|| unsafe { button.CurrentAutomationId() })? != "1" {
                continue;
            }
            let name = self.call(|| unsafe { button.CurrentName() })?.to_string();
            if !button_caption(&name) {
                continue;
            }
            self.guarded(&button)?;
            let invoke =
                self.call(|| unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId) })?;
            matches.push((button, invoke));
        }
        if matches.len() != 1 {
            return Err(Failure::provider(
                "named Open/Save IDOK is missing or ambiguous",
            ));
        }
        matches
            .pop()
            .ok_or_else(|| Failure::provider("IDOK missing"))
    }
}
impl Dialog for Adapter {
    fn snapshot(&mut self) -> ProviderResult<Snapshot> {
        if !navigation::is_local_absolute(&self.request.target)
            || !std::path::Path::new(&self.request.target).is_dir()
        {
            return Err(Failure::provider(
                "target is not an existing local absolute directory",
            ));
        }
        let _ = self.command()?;
        Ok(Snapshot {
            folder: self.folder()?,
            filename: self.filename()?,
            file_type: self.file_type()?,
        })
    }
    fn write_filename(&mut self, permitted: &[&str], text: &str) -> ProviderResult<()> {
        let (edit, value) = self.filename_control()?;
        let current = self.call(|| unsafe { value.CurrentValue() })?.to_string();
        if !permitted.contains(&current.as_str()) {
            return Err(Failure::interrupted(
                "pending filename was edited; refusing to overwrite it",
            ));
        }
        self.guarded(&edit)?;
        self.call(|| unsafe { value.SetValue(&BSTR::from(text)) })
    }
    fn invoke(&mut self) -> ProviderResult<()> {
        if !std::path::Path::new(&self.request.target).is_dir() {
            return Err(Failure::interrupted(
                "target directory disappeared before Invoke",
            ));
        }
        let (button, invoke) = self.command()?;
        let expected = format!("{}\\", self.request.target.trim_end_matches('\\'));
        if self.filename()? != expected {
            return Err(Failure::interrupted(
                "pending filename changed before Invoke",
            ));
        }
        self.guarded(&button)?;
        self.call(|| unsafe { invoke.Invoke() })
    }
    fn filename(&mut self) -> ProviderResult<String> {
        let (_, value) = self.filename_control()?;
        Ok(self.call(|| unsafe { value.CurrentValue() })?.to_string())
    }
    fn file_type(&mut self) -> ProviderResult<String> {
        let mut matches = Vec::new();
        let mut observed_ids = Vec::new();
        for combo in self.descendants(&[UIA_ComboBoxControlTypeId.0])? {
            let id = self
                .call(|| unsafe { combo.CurrentAutomationId() })?
                .to_string();
            let name = self.call(|| unsafe { combo.CurrentName() })?.to_string();
            observed_ids.push((id.clone(), name.clone()));
            // The same modern IFileDialog picker has a native ComboBox (1136)
            // representation on this build. Bind one semantic control, not a
            // fallback getter; reject ambiguous matches before any mutation.
            let label = name.to_lowercase();
            let native_type = id == "1136"
                && (label.contains("文件类型")
                    || label.contains("保存类型")
                    || label.contains("file type")
                    || label.contains("files of type")
                    || label.contains("save as type"));
            if id != "FileTypeControlHost" && !native_type {
                continue;
            }
            self.guarded(&combo)?;
            let value: IUIAutomationValuePattern =
                self.call(|| unsafe { combo.GetCurrentPatternAs(UIA_ValuePatternId) })?;
            let selected = self.call(|| unsafe { value.CurrentValue() })?.to_string();
            if selected.trim().is_empty() {
                return Err(Failure::provider(
                    "selected file type is empty or ambiguous",
                ));
            }
            matches.push(selected);
        }
        if matches.len() != 1 {
            return Err(Failure::provider(format!(
                "selected file type control is missing or ambiguous (matches={}, combo_ids={observed_ids:?})",
                matches.len()
            )));
        }
        matches
            .pop()
            .ok_or_else(|| Failure::provider("selected file type unavailable"))
    }
    fn folder(&mut self) -> ProviderResult<String> {
        self.check()?;
        unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> BOOL {
            let children = unsafe { &mut *(param.0 as *mut Vec<HWND>) };
            if children.len() >= 256 {
                return BOOL(0);
            }
            children.push(window);
            BOOL(1)
        }
        let mut children = Vec::new();
        unsafe {
            let _ = EnumChildWindows(
                Some(self.window()),
                Some(visit),
                LPARAM((&raw mut children) as isize),
            );
        }
        if children.len() >= 256 {
            return Err(Failure::provider("native child lookup exceeded bound"));
        }
        let mut paths = Vec::new();
        for child in children {
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(child, Some(&mut pid));
            }
            if class_name(child) != "ToolbarWindow32"
                || pid != self.request.dialog.pid
                || !unsafe { IsWindowVisible(child).as_bool() }
            {
                continue;
            }
            let Ok(parent) = (unsafe { GetParent(child) }) else {
                continue;
            };
            if class_name(parent) != "Breadcrumb Parent" {
                continue;
            }
            let (mut cursor, mut band, mut anchored) = (parent, false, false);
            for _ in 0..16 {
                let mut owner = 0;
                unsafe {
                    GetWindowThreadProcessId(cursor, Some(&mut owner));
                }
                if owner != self.request.dialog.pid || !unsafe { IsWindow(Some(cursor)).as_bool() }
                {
                    break;
                }
                band |= class_name(cursor) == "Address Band Root";
                if cursor == self.window() {
                    anchored = true;
                    break;
                }
                let Ok(next) = (unsafe { GetParent(cursor) }) else {
                    break;
                };
                cursor = next;
            }
            if !band || !anchored {
                continue;
            }
            let element = self.call(|| unsafe { self.uia.ElementFromHandle(child) })?;
            if self.call(|| unsafe { element.CurrentAutomationId() })? != "1001"
                || self.call(|| unsafe { element.CurrentNativeWindowHandle() })? != child
            {
                return Err(Failure::interrupted("toolbar binding changed"));
            }
            self.guarded(&element)?;
            let name = self.call(|| unsafe { element.CurrentName() })?.to_string();
            paths.push(navigation::address_path(&name).ok_or_else(|| {
                Failure::provider("address toolbar does not expose a supported complete path")
            })?);
        }
        if paths.len() != 1 {
            return Err(Failure::provider(
                "complete-path address toolbar is missing or ambiguous",
            ));
        }
        paths
            .pop()
            .ok_or_else(|| Failure::provider("folder unavailable"))
    }
    fn validate(&mut self) -> ProviderResult<()> {
        self.check()
    }
}
fn class_name(window: HWND) -> String {
    let mut text = [0_u16; 128];
    let count = unsafe { GetClassNameW(window, &mut text) };
    usize::try_from(count)
        .ok()
        .map_or_else(String::new, |n| String::from_utf16_lossy(&text[..n]))
}
fn button_caption(name: &str) -> bool {
    let name = name
        .split(['(', '（', '\t'])
        .next()
        .unwrap_or_default()
        .replace('&', "");
    matches!(
        name.trim().to_lowercase().as_str(),
        "打开" | "保存" | "open" | "save"
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_invoke_other_idok_commands() {
        assert!(button_caption("打开(&O)"));
        assert!(button_caption("Save (&S)"));
        assert!(!button_caption("Open with"));
        assert!(!button_caption("保存副本"));
        assert!(!button_caption("Cancel"));
    }
}
