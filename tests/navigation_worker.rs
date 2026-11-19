//! Real product entry-point/protocol check; creates no dialog and injects no input.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use pathwarp::core::navigation::{DialogIdentity, Outcome, Request};
use pathwarp::os::dialog::WorkerMessage;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

#[test]
fn worker_dispatches_before_gui_and_rejects_stale_target_without_actions() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_PathWarp"))
        .arg("--navigation-worker")
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let request = Request {
        id: 17,
        session: 23,
        dialog: DialogIdentity {
            hwnd: 0,
            pid: 0,
            process_created: 0,
        },
        overlay_hwnd: 0,
        target: "C:\\fixture".into(),
    };
    let mut input = child.stdin.take().unwrap();
    serde_json::to_writer(&mut input, &request).unwrap();
    writeln!(input).unwrap();
    drop(input);
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() >= Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("worker dispatch must not start the GUI event loop");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let messages: Vec<WorkerMessage> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(messages.len(), 1);
    let WorkerMessage::Complete(report) = &messages[0] else {
        panic!("final report missing");
    };
    assert_eq!(report.request, request);
    assert_eq!(report.outcome, Outcome::Interrupted);
    assert_eq!(report.writes, 0);
    assert_eq!(report.invokes, 0);
    assert!(report.original.is_none());
}
