//! Own-worker supervision. Target dialogs are never closed or rolled back.
mod uia;
use crate::core::navigation::{Outcome, Report, Request, SUPERVISOR_BUDGET};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", content = "report")]
pub enum WorkerMessage {
    Progress(Report),
    Complete(Report),
}
struct Running {
    request: Request,
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Report>,
    supervisor: std::thread::JoinHandle<()>,
}
#[derive(Default)]
pub struct Navigator {
    running: Option<Running>,
    blocked: bool,
    executable: Option<std::path::PathBuf>,
}
impl Navigator {
    /// Use the product executable from an owned acceptance runner.
    pub fn with_worker(executable: std::path::PathBuf) -> Self {
        Self {
            executable: Some(executable),
            running: None,
            blocked: false,
        }
    }
    pub fn start(&mut self, request: Request) -> bool {
        if self.running.is_some() || self.blocked {
            return false;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_request = request.clone();
        let executable = self.executable.clone();
        let supervisor = std::thread::spawn(move || {
            let report = supervise(worker_request, &worker_cancel, executable);
            let _ = sender.send(report);
        });
        self.running = Some(Running {
            request,
            cancel,
            receiver,
            supervisor,
        });
        true
    }
    pub fn fence(&self, session: Option<u64>, foreground: isize) {
        if let Some(running) = &self.running
            && (session != Some(running.request.session)
                || (foreground != running.request.dialog.hwnd
                    && foreground != running.request.overlay_hwnd))
        {
            running.cancel.store(true, Ordering::Release);
        }
    }
    pub fn poll(&mut self) -> Option<Report> {
        let running = self.running.as_ref()?;
        let report = match running.receiver.try_recv() {
            Ok(report) => report,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                let mut report = Report::new(running.request.clone());
                report.outcome = Outcome::Unverified;
                report.reason = "navigation supervisor disconnected; effect is unknown".into();
                report
            }
        };
        self.blocked = !report.worker_retired;
        self.running = None;
        Some(report)
    }
}
impl Drop for Navigator {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.cancel.store(true, Ordering::Release);
            // Closing the app must not abandon a worker which can still mutate.
            let _ = running.supervisor.join();
        }
    }
}
fn supervise(
    request: Request,
    cancel: &AtomicBool,
    selected: Option<std::path::PathBuf>,
) -> Report {
    let executable = match selected.map_or_else(std::env::current_exe, Ok) {
        Ok(path) => path,
        Err(error) => {
            let mut report = Report::new(request);
            report.reason = format!("worker executable unavailable: {error}");
            report.worker_retired = true;
            return report;
        }
    };
    let mut command = Command::new(executable);
    command.arg("--navigation-worker");
    supervise_command(request, cancel, command, SUPERVISOR_BUDGET)
}

fn retire(child: &mut std::process::Child) -> bool {
    if child.try_wait().ok().flatten().is_some() {
        return true;
    }
    if child.kill().is_ok() {
        child.wait().is_ok()
    } else {
        child.try_wait().ok().flatten().is_some()
    }
}

fn supervise_command(
    request: Request,
    cancel: &AtomicBool,
    mut command: Command,
    budget: Duration,
) -> Report {
    let mut latest = Report::new(request.clone());
    latest.worker_retired = true;
    if cancel.load(Ordering::Acquire) {
        latest.outcome = Outcome::Interrupted;
        latest.reason = "request canceled before worker launch".into();
        return latest;
    }
    let started = Instant::now();
    let spawn = (|| -> std::io::Result<_> {
        let mut child = command
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        latest.worker_retired = false;
        let write = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("missing worker stdin"))
            .and_then(|mut input| {
                serde_json::to_writer(&mut input, &request)?;
                writeln!(input)
            });
        if let Err(error) = write {
            latest.worker_retired = retire(&mut child);
            return Err(error);
        }
        Ok(child)
    })();
    let mut child = match spawn {
        Ok(child) => child,
        Err(error) => {
            latest.reason = format!("worker could not start: {error}");
            return latest;
        }
    };
    let (sender, receiver) = mpsc::channel();
    let mut reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.len() > 65_536 {
                    break;
                }
                if let Ok(message) = serde_json::from_str::<WorkerMessage>(&line)
                    && sender.send(message).is_err()
                {
                    break;
                }
            }
        })
    });
    let mut completed = None;
    loop {
        let mut invalid_message = false;
        for message in receiver.try_iter() {
            let report = match message {
                WorkerMessage::Progress(report) => report,
                WorkerMessage::Complete(report) => {
                    completed = Some(report.clone());
                    report
                }
            };
            if report.request != request {
                invalid_message = true;
                completed = None;
                break;
            }
            latest = report;
        }
        let canceled = cancel.load(Ordering::Acquire);
        if invalid_message || canceled || started.elapsed() >= budget {
            latest.worker_retired = retire(&mut child);
            latest.outcome = if canceled {
                Outcome::Interrupted
            } else {
                Outcome::Unverified
            };
            latest.reason = if canceled {
                "interaction invalidated; pending provider effect is unknown"
            } else {
                "worker timed out or protocol failed; pending provider effect is unknown"
            }
            .into();
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(reader) = reader.take() {
                    let _ = reader.join();
                }
                for message in receiver.try_iter() {
                    match message {
                        WorkerMessage::Complete(report) if report.request == request => {
                            completed = Some(report)
                        }
                        WorkerMessage::Progress(report) if report.request == request => {
                            latest = report
                        }
                        _ => {}
                    }
                }
                if status.success()
                    && let Some(mut report) = completed
                {
                    report.worker_retired = true;
                    return report;
                }
                latest.outcome = Outcome::Unverified;
                latest.worker_retired = true;
                latest.reason =
                    "worker exited without a matching completion; effect is unknown".into();
                return latest;
            }
            Err(error) => {
                latest.worker_retired = retire(&mut child);
                latest.outcome = Outcome::Unverified;
                latest.reason = format!("worker status failed: {error}");
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    if latest.worker_retired
        && let Some(reader) = reader
    {
        let _ = reader.join();
    }
    latest
}
/// Called before logging/eframe initialization so a worker never creates an overlay.
#[allow(clippy::print_stdout)]
pub fn run_worker_if_requested() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new("--navigation-worker")) {
        return false;
    }
    let input = std::io::stdin().lock().lines().next();
    let report = match input
        .and_then(Result::ok)
        .filter(|line| line.len() <= 65_536)
        .and_then(|line| serde_json::from_str::<Request>(&line).ok())
    {
        Some(request) => uia::run(request, |report| {
            if let Ok(line) = serde_json::to_string(&WorkerMessage::Progress(report.clone())) {
                println!("{line}");
            }
        }),
        None => return true,
    };
    if let Ok(line) = serde_json::to_string(&WorkerMessage::Complete(report)) {
        println!("{line}");
    }
    true
}
pub fn identity(handle: isize) -> Option<crate::core::navigation::DialogIdentity> {
    uia::identity(handle)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)]
mod tests {
    use super::*;
    use crate::core::navigation::{DialogIdentity, Snapshot, Stage};
    fn request() -> Request {
        Request {
            id: 1,
            session: 2,
            dialog: DialogIdentity {
                hwnd: 3,
                pid: 4,
                process_created: 5,
            },
            overlay_hwnd: 0,
            target: "C:\\B".into(),
        }
    }
    #[test]
    #[ignore = "owned blocking child used only by the supervisor timeout test"]
    fn blocking_worker_fixture() {
        if std::env::var("PATHWARP_SUPERVISOR_FIXTURE").as_deref() != Ok("1") {
            return;
        }
        let line = std::io::stdin().lock().lines().next().unwrap().unwrap();
        let request: Request = serde_json::from_str(&line).unwrap();
        if let Ok(path) = std::env::var("PATHWARP_SUPERVISOR_MARKER") {
            let mut marker = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            write!(marker, "{}", std::process::id()).unwrap();
        }
        let mut report = Report::new(request);
        report.original = Some(Snapshot {
            folder: "C:\\A".into(),
            filename: "pending.csv".into(),
            file_type: "CSV".into(),
        });
        report.stage = Stage::Assign;
        report.writes = 1;
        println!(
            "\n{}",
            serde_json::to_string(&WorkerMessage::Progress(report)).unwrap()
        );
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
    #[test]
    fn timeout_retires_only_owned_worker_and_keeps_original_snapshot() {
        let marker = std::env::temp_dir().join(format!(
            "pathwarp-supervisor-{}-{}.pid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "os::dialog::tests::blocking_worker_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("PATHWARP_SUPERVISOR_FIXTURE", "1")
            .env("PATHWARP_SUPERVISOR_MARKER", &marker);
        let started = Instant::now();
        let report = supervise_command(
            request(),
            &AtomicBool::new(false),
            command,
            Duration::from_secs(2),
        );
        assert!(started.elapsed() < Duration::from_secs(8));
        assert_eq!(report.outcome, Outcome::Unverified);
        assert!(report.worker_retired);
        assert_eq!(report.stage, Stage::Assign);
        assert_eq!(report.writes, 1);
        assert_eq!(report.original.unwrap().filename, "pending.csv");
        let pid: u32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
        std::fs::remove_file(marker).unwrap();
        if let Ok(handle) = unsafe {
            windows::Win32::System::Threading::OpenProcess(
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        } {
            let mut code = 0;
            let result =
                unsafe { windows::Win32::System::Threading::GetExitCodeProcess(handle, &mut code) };
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
            result.unwrap();
            assert_ne!(code, 259, "owned worker must no longer be running");
        }
    }
    #[test]
    fn canceled_request_never_launches_a_worker() {
        let command = Command::new("missing-worker-must-not-be-launched.exe");
        let report = supervise_command(
            request(),
            &AtomicBool::new(true),
            command,
            Duration::from_secs(1),
        );
        assert_eq!(report.outcome, Outcome::Interrupted);
        assert!(report.worker_retired);
        assert!(!report.may_have_changed());
    }
}
