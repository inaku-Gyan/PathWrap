#![cfg(windows)]
#![allow(clippy::unwrap_used)]
use pathwarp::startup::Target;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn startup_command_round_trips_through_actual_rust_windows_argv() {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_startup_argv_probe"));
    for storage in [r"C:\中文 设置\", r"C:\", r"\\server\share\配置\"] {
        let target = Target {
            executable: executable.clone(),
            storage: storage.into(),
        };
        let command = target.command().unwrap();
        let arguments = command
            .strip_prefix(&format!("\"{}\" ", executable.display()))
            .unwrap();
        let output = Command::new(&executable)
            .raw_arg(arguments)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), storage);
    }
}
