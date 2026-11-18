//! One folder-jump transaction. Provider calls live behind a narrow adapter;
//! neither a successful call nor assigned filename text constitutes arrival.
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const ARRIVAL_BUDGET: Duration = Duration::from_secs(3);
pub const WORKER_BUDGET: Duration = Duration::from_secs(12);
pub const SUPERVISOR_BUDGET: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogIdentity {
    pub hwnd: isize,
    pub pid: u32,
    pub process_created: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub session: u64,
    pub dialog: DialogIdentity,
    pub overlay_hwnd: isize,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Preflight,
    Assign,
    Invoke,
    Arrival,
    Reassert,
    Verify,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Jumped,
    AlreadyAtTarget,
    Unsupported,
    Unverified,
    PreservationFailed,
    Interrupted,
}

impl Outcome {
    pub fn is_success(self) -> bool {
        matches!(self, Self::Jumped | Self::AlreadyAtTarget)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub folder: String,
    pub filename: String,
    pub file_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub request: Request,
    pub outcome: Outcome,
    pub stage: Stage,
    pub reason: String,
    pub original: Option<Snapshot>,
    pub folder: Option<String>,
    pub filename: Option<String>,
    pub file_type: Option<String>,
    /// Counts attempts, including calls which fail or never return.
    pub writes: u8,
    pub invokes: u8,
    /// Set only by the supervisor after confirming its child has exited.
    #[serde(default)]
    pub worker_retired: bool,
}

impl Report {
    pub fn new(request: Request) -> Self {
        Self {
            request,
            outcome: Outcome::Unsupported,
            stage: Stage::Preflight,
            reason: String::new(),
            original: None,
            folder: None,
            filename: None,
            file_type: None,
            writes: 0,
            invokes: 0,
            worker_retired: false,
        }
    }

    pub fn may_have_changed(&self) -> bool {
        self.writes != 0 || self.invokes != 0
    }

    pub fn failure_message(&self) -> String {
        let message = match self.outcome {
            Outcome::Unsupported => "此对话框暂不支持安全跳转，未执行写入。",
            Outcome::Interrupted => "操作已中断，请检查原对话框的当前状态。",
            Outcome::Unverified => "无法确认跳转结果，目录或文件名可能已改变。",
            Outcome::PreservationFailed => "原文件名或文件类型未通过保留核验，请检查对话框。",
            Outcome::Jumped | Outcome::AlreadyAtTarget => return String::new(),
        };
        match &self.original {
            Some(original) if self.may_have_changed() => {
                format!("{message}\n原文件名：{}", original.filename)
            }
            _ => message.to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Failure {
    pub reason: String,
    pub interrupted: bool,
}

impl Failure {
    pub fn provider(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            interrupted: false,
        }
    }
    pub fn interrupted(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            interrupted: true,
        }
    }
}

pub type ProviderResult<T> = Result<T, Failure>;

pub trait Dialog {
    /// Includes target identity, foreground ownership, deadline and button guards.
    fn snapshot(&mut self) -> ProviderResult<Snapshot>;
    fn write_filename(&mut self, permitted: &[&str], text: &str) -> ProviderResult<()>;
    fn invoke(&mut self) -> ProviderResult<()>;
    fn folder(&mut self) -> ProviderResult<String>;
    fn filename(&mut self) -> ProviderResult<String>;
    fn file_type(&mut self) -> ProviderResult<String>;
    fn validate(&mut self) -> ProviderResult<()>;
    fn wait(&mut self) {
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn is_local_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && !path.contains(['/', '\0', '\r', '\n'])
        && !path[3..].split('\\').any(|part| matches!(part, "." | ".."))
}

pub fn same_folder(left: &str, right: &str) -> bool {
    is_local_absolute(left)
        && is_local_absolute(right)
        && left
            .trim_end_matches('\\')
            .eq_ignore_ascii_case(right.trim_end_matches('\\'))
}

pub fn address_path(name: &str) -> Option<String> {
    let name = name.trim().trim_start_matches('\u{200e}').trim();
    ["地址: ", "Address: "].iter().find_map(|prefix| {
        let path = name.strip_prefix(prefix)?;
        is_local_absolute(path).then(|| path.to_owned())
    })
}

fn single_filename(text: &str) -> bool {
    !text.trim().is_empty()
        && !matches!(text, "." | "..")
        && !text.contains([
            '"', ';', '\0', '\r', '\n', '\\', '/', ':', '*', '?', '<', '>', '|',
        ])
}

pub fn execute(
    dialog: &mut impl Dialog,
    request: Request,
    mut progress: impl FnMut(&Report),
) -> Report {
    let mut report = Report::new(request);
    let result = transaction(dialog, &mut report, &mut progress);
    if let Err(error) = result {
        report.outcome = if error.interrupted {
            Outcome::Interrupted
        } else if !report.may_have_changed() {
            Outcome::Unsupported
        } else if matches!(report.stage, Stage::Reassert | Stage::Verify) {
            Outcome::PreservationFailed
        } else {
            Outcome::Unverified
        };
        report.reason = error.reason;
    }
    report
}

fn transaction(
    dialog: &mut impl Dialog,
    report: &mut Report,
    progress: &mut impl FnMut(&Report),
) -> ProviderResult<()> {
    if !is_local_absolute(&report.request.target) {
        return Err(Failure::provider(
            "target is not a supported local absolute directory",
        ));
    }
    let original = dialog.snapshot()?;
    if !is_local_absolute(&original.folder)
        || !single_filename(&original.filename)
        || original.file_type.trim().is_empty()
    {
        return Err(Failure::provider(
            "folder, single pending filename or selected type is unavailable",
        ));
    }
    report.folder = Some(original.folder.clone());
    report.filename = Some(original.filename.clone());
    report.file_type = Some(original.file_type.clone());
    report.original = Some(original.clone());
    progress(report);
    if dialog.snapshot()? != original {
        return Err(Failure::interrupted(
            "pending operation changed during preflight",
        ));
    }
    if same_folder(&original.folder, &report.request.target) {
        dialog.validate()?;
        report.outcome = Outcome::AlreadyAtTarget;
        report.stage = Stage::Complete;
        return Ok(());
    }
    let target = format!("{}\\", report.request.target.trim_end_matches('\\'));
    report.stage = Stage::Assign;
    report.writes += 1;
    progress(report);
    dialog.write_filename(&[&original.filename], &target)?;
    let assigned = dialog.filename()?;
    report.filename = Some(assigned.clone());
    if assigned != target {
        return Err(Failure::interrupted(
            "assigned filename changed before Invoke",
        ));
    }
    report.file_type = Some(dialog.file_type()?);
    report.folder = Some(dialog.folder()?);
    if report.file_type.as_deref() != Some(&original.file_type)
        || !report
            .folder
            .as_ref()
            .is_some_and(|folder| same_folder(folder, &original.folder))
    {
        return Err(Failure::interrupted(
            "folder or selected type changed before Invoke",
        ));
    }
    report.stage = Stage::Invoke;
    report.invokes += 1;
    progress(report);
    dialog.invoke()?;
    report.stage = Stage::Arrival;
    let started = Instant::now();
    loop {
        let folder = dialog.folder()?;
        report.folder = Some(folder.clone());
        progress(report);
        if same_folder(&folder, &report.request.target) {
            break;
        }
        if !same_folder(&folder, &original.folder) {
            return Err(Failure::interrupted(
                "another folder was observed during navigation",
            ));
        }
        if started.elapsed() >= ARRIVAL_BUDGET {
            return Err(Failure::provider(
                "B arrival was not established within the observation budget",
            ));
        }
        dialog.wait();
    }
    report.stage = Stage::Reassert;
    let selected_type = dialog.file_type()?;
    report.file_type = Some(selected_type.clone());
    if selected_type != original.file_type {
        return Err(Failure::provider(
            "selected file type changed; no restoration action is permitted",
        ));
    }
    report.writes += 1;
    progress(report);
    dialog.write_filename(&[&original.filename, &target], &original.filename)?;
    report.stage = Stage::Verify;
    report.filename = Some(dialog.filename()?);
    report.file_type = Some(dialog.file_type()?);
    report.folder = Some(dialog.folder()?);
    dialog.validate()?;
    if report.filename.as_deref() != Some(&original.filename)
        || report.file_type.as_deref() != Some(&original.file_type)
        || !report
            .folder
            .as_ref()
            .is_some_and(|folder| same_folder(folder, &report.request.target))
    {
        return Err(Failure::provider(
            "final arrival or pending-operation preservation check failed",
        ));
    }
    report.stage = Stage::Complete;
    report.outcome = Outcome::Jumped;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    struct Fake {
        state: Snapshot,
        writes: Vec<String>,
        invokes: usize,
        invoke_error: bool,
        drift_type: bool,
        reassert_error: bool,
        interference: bool,
        missing_type: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                state: Snapshot {
                    folder: "C:\\A".into(),
                    filename: "pending.csv".into(),
                    file_type: "CSV".into(),
                },
                writes: vec![],
                invokes: 0,
                invoke_error: false,
                drift_type: false,
                reassert_error: false,
                interference: false,
                missing_type: false,
            }
        }
    }
    impl Dialog for Fake {
        fn snapshot(&mut self) -> ProviderResult<Snapshot> {
            if self.missing_type {
                return Err(Failure::provider("no selected type"));
            }
            Ok(self.state.clone())
        }
        fn write_filename(&mut self, permitted: &[&str], text: &str) -> ProviderResult<()> {
            if !permitted.contains(&self.state.filename.as_str()) {
                return Err(Failure::interrupted("user edit"));
            }
            if self.reassert_error && !self.writes.is_empty() {
                return Err(Failure::provider("restore denied"));
            }
            self.writes.push(text.to_owned());
            self.state.filename = text.to_owned();
            Ok(())
        }
        fn invoke(&mut self) -> ProviderResult<()> {
            self.invokes += 1;
            if self.invoke_error {
                return Err(Failure::provider("Invoke error"));
            }
            self.state.folder = "C:\\B".into();
            if self.drift_type {
                self.state.file_type = "Text".into();
            }
            if self.interference {
                self.state.filename = "user-edited.csv".into();
            }
            Ok(())
        }
        fn folder(&mut self) -> ProviderResult<String> {
            Ok(self.state.folder.clone())
        }
        fn filename(&mut self) -> ProviderResult<String> {
            Ok(self.state.filename.clone())
        }
        fn file_type(&mut self) -> ProviderResult<String> {
            Ok(self.state.file_type.clone())
        }
        fn validate(&mut self) -> ProviderResult<()> {
            Ok(())
        }
    }
    fn request() -> Request {
        Request {
            id: 1,
            session: 7,
            dialog: DialogIdentity {
                hwnd: 1,
                pid: 2,
                process_created: 3,
            },
            overlay_hwnd: 0,
            target: "C:\\B".into(),
        }
    }
    #[test]
    fn real_reassertion_preserves_pending_operation() {
        let mut fake = Fake::default();
        let result = execute(&mut fake, request(), |_| {});
        assert_eq!(result.outcome, Outcome::Jumped);
        assert_eq!(fake.writes, ["C:\\B\\", "pending.csv"]);
        assert_eq!(fake.invokes, 1);
        assert_eq!(result.filename.as_deref(), Some("pending.csv"));
    }
    #[test]
    fn missing_type_and_already_at_target_do_not_mutate() {
        let mut fake = Fake {
            missing_type: true,
            ..Fake::default()
        };
        assert_eq!(
            execute(&mut fake, request(), |_| {}).outcome,
            Outcome::Unsupported
        );
        assert!(fake.writes.is_empty());
        assert_eq!(fake.invokes, 0);
        fake.missing_type = false;
        fake.state.folder = "C:\\B".into();
        assert_eq!(
            execute(&mut fake, request(), |_| {}).outcome,
            Outcome::AlreadyAtTarget
        );
        assert!(fake.writes.is_empty());
        assert_eq!(fake.invokes, 0);
    }
    #[test]
    fn invoke_error_does_not_claim_no_effect_or_attempt_cleanup() {
        let mut fake = Fake {
            invoke_error: true,
            ..Fake::default()
        };
        let result = execute(&mut fake, request(), |_| {});
        assert_eq!(result.outcome, Outcome::Unverified);
        assert_eq!(fake.writes, ["C:\\B\\"]);
        assert_eq!(fake.invokes, 1);
        assert!(result.may_have_changed());
    }
    #[test]
    fn type_drift_and_user_edits_are_not_overwritten() {
        let mut fake = Fake {
            drift_type: true,
            ..Fake::default()
        };
        let result = execute(&mut fake, request(), |_| {});
        assert_eq!(result.outcome, Outcome::PreservationFailed);
        assert_eq!(fake.writes.len(), 1);
        let mut fake = Fake {
            interference: true,
            ..Fake::default()
        };
        assert_eq!(
            execute(&mut fake, request(), |_| {}).outcome,
            Outcome::Interrupted
        );
        assert_eq!(fake.state.filename, "user-edited.csv");
    }
    #[test]
    fn restore_failure_keeps_verified_folder_and_original_filename() {
        let mut fake = Fake {
            reassert_error: true,
            ..Fake::default()
        };
        let result = execute(&mut fake, request(), |_| {});
        assert_eq!(result.outcome, Outcome::PreservationFailed);
        assert_eq!(result.folder.as_deref(), Some("C:\\B"));
        assert_eq!(result.original.unwrap().filename, "pending.csv");
        assert_eq!(fake.writes.len(), 1);
    }
    #[test]
    fn path_matching_cannot_accept_basename_prefix_or_relative_path() {
        assert!(same_folder("C:\\B\\", "c:\\b"));
        assert!(!same_folder("C:\\B2", "C:\\B"));
        assert!(!same_folder("C:", "C:\\"));
        assert!(!is_local_absolute("C:\\A\\..\\B"));
        assert_eq!(address_path("\u{200e}地址: C:\\B"), Some("C:\\B".into()));
        assert!(address_path("B").is_none());
    }
}
