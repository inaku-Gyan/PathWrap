//! One current-user registration, independent of theme persistence. Observations
//! describe the Run value only, never Windows' permission to launch it.
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub executable: PathBuf,
    pub storage: PathBuf,
}

impl Target {
    /// Freeze fully qualified paths without assuming aliases are equivalent.
    pub fn current(executable: &Path, storage: &Path) -> Result<Self, String> {
        if !executable.is_file() || !storage.is_dir() {
            return Err("程序或配置目录不存在，无法确认登录启动目标。".into());
        }
        let target = Self {
            executable: full_path(executable)?,
            storage: full_path(storage)?,
        };
        target.command()?;
        Ok(target)
    }

    pub fn command(&self) -> Result<String, String> {
        let executable = path_text(&self.executable)?;
        let storage = path_text(&self.storage)?;
        let command = format!("{} --storage-dir {}", quote(executable), quote(storage));
        // Include the terminating NUL in this conservative budget.
        if command.encode_utf16().count() + 1 > 260 {
            return Err("完整登录启动命令超过 260 字符预算；原登记保持不变。".into());
        }
        Ok(command)
    }

    pub fn paths_exist(&self) -> bool {
        self.executable.is_file() && self.storage.is_dir()
    }
}

fn full_path(path: &Path) -> Result<PathBuf, String> {
    path_text(path)?;
    std::path::absolute(path).map_err(|error| format!("无法确认完整路径：{error}"))
}

fn path_text(path: &Path) -> Result<&str, String> {
    let text = path.to_str().ok_or("路径不能可靠编码为 Unicode。")?;
    if !path.is_absolute()
        || text.starts_with(r"\\?\")
        || text.starts_with(r"\\.\")
        || text.chars().any(|ch| ch == '"' || ch.is_control())
    {
        return Err("路径不是可确认的完整普通路径；不登记驱动器相对、根相对或设备路径。".into());
    }
    Ok(text)
}

/// Rust's Windows argv quoting: a final run of backslashes is doubled before
/// the closing quote. Path validation excludes embedded quotes and NULs.
fn quote(text: &str) -> String {
    let trailing = text.chars().rev().take_while(|ch| *ch == '\\').count();
    format!("\"{text}{}\"", "\\".repeat(trailing))
}

fn quoted_path(input: &str) -> Option<(String, &str)> {
    let body = input.strip_prefix('"')?;
    let end = body.find('"')?;
    let encoded = &body[..end];
    let trailing = encoded.chars().rev().take_while(|ch| *ch == '\\').count();
    if trailing % 2 != 0 {
        return None;
    }
    let decoded = &encoded[..encoded.len() - trailing / 2];
    Some((decoded.to_owned(), &body[end + 1..]))
}

/// Recognize only our canonical, fully quoted command form. Ambiguous or extra
/// arguments remain read-only. No shell or environment expansion is performed.
fn recognize(command: &str, current: Option<&Target>) -> Option<Target> {
    let (executable, rest) = quoted_path(command)?;
    let (storage, rest) = quoted_path(rest.strip_prefix(" --storage-dir ")?)?;
    if !rest.is_empty() {
        return None;
    }
    let target = Target {
        executable: executable.into(),
        storage: storage.into(),
    };
    let name = target.executable.file_name()?.to_str()?;
    let current_name = current
        .and_then(|target| target.executable.file_name())
        .and_then(|name| name.to_str());
    if !name.eq_ignore_ascii_case("PathWarp.exe")
        && !current_name.is_some_and(|current| name.eq_ignore_ascii_case(current))
    {
        return None;
    }
    (target.command().ok()?.as_str() == command).then_some(target)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawRegistration {
    Absent,
    Value { kind: u32, bytes: Vec<u8> },
}

impl RawRegistration {
    pub fn command(command: &str) -> Self {
        Self::Value {
            kind: 1, // REG_SZ, deliberately not REG_EXPAND_SZ.
            bytes: command
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect(),
        }
    }

    fn text(&self) -> Option<String> {
        let Self::Value { kind: 1, bytes } = self else {
            return None;
        };
        if bytes.len() % 2 != 0 || bytes.len() > 4096 {
            return None;
        }
        let mut wide: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        if wide.pop() != Some(0) || wide.contains(&0) {
            return None;
        }
        String::from_utf16(&wide).ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    Absent,
    Recognized {
        target: Target,
        current: bool,
        exists: bool,
    },
    Conflict(String),
    Unreadable(String),
}

impl Observation {
    pub fn writable(&self) -> bool {
        matches!(self, Self::Absent | Self::Recognized { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    UseCurrent,
    Remove,
}

/// Locks serialize explicit actions by our own instances. External editors are
/// not covered; compare a fresh snapshot immediately before every mutation.
pub trait Backend {
    fn lock(&self) -> Result<Box<dyn std::any::Any>, String>;
    fn read(&self) -> Result<RawRegistration, String>;
    fn write(&self, value: &RawRegistration) -> Result<(), String>;
    fn validate_target(&self, target: &Target) -> Result<(), String>;
}

pub struct Registration<B> {
    backend: B,
    pub target: Result<Target, String>,
    pub observation: Observation,
    pub message: String,
    pub revision: u64,
    pub unconfirmed: bool,
    pub last_target: Option<Target>,
    snapshot: Option<RawRegistration>,
}

impl<B: Backend> Registration<B> {
    pub fn new(backend: B, target: Result<Target, String>) -> Self {
        let mut registration = Self {
            backend,
            target,
            observation: Observation::Absent,
            message: String::new(),
            revision: 0,
            unconfirmed: false,
            last_target: None,
            snapshot: None,
        };
        registration.refresh(); // Startup is read-only.
        registration
    }

    fn observe(&mut self, result: Result<RawRegistration, String>) {
        self.revision = self.revision.wrapping_add(1);
        self.observation = match &result {
            Ok(RawRegistration::Absent) => Observation::Absent,
            Ok(raw) => match raw
                .text()
                .and_then(|text| recognize(&text, self.target.as_ref().ok()))
            {
                Some(target) => Observation::Recognized {
                    current: self.target.as_ref().ok() == Some(&target),
                    exists: target.paths_exist(),
                    target,
                },
                None => {
                    Observation::Conflict("同名登记的类型、命令或归属无法识别；保持只读。".into())
                }
            },
            Err(error) => Observation::Unreadable(error.clone()),
        };
        if let Observation::Recognized { target, .. } = &self.observation {
            self.last_target = Some(target.clone());
        }
        self.snapshot = result.ok();
    }

    pub fn refresh(&mut self) {
        self.observe(self.backend.read());
        if !matches!(self.observation, Observation::Unreadable(_)) {
            self.unconfirmed = false;
            self.message.clear();
        }
    }

    pub fn execute(&mut self, action: Action, expected_revision: u64) {
        if self.unconfirmed {
            self.message = "结果未确认，请先刷新登记。".into();
            return;
        }
        if expected_revision != self.revision || !self.observation.writable() {
            self.message = "登记状态已经变化或不可修改，请刷新后重新选择。".into();
            return;
        }
        let desired = match action {
            Action::UseCurrent => {
                match self
                    .target
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|target| {
                        self.backend.validate_target(target)?;
                        target.command()
                    }) {
                    Ok(command) => RawRegistration::command(&command),
                    Err(error) => {
                        self.message = error;
                        return;
                    }
                }
            }
            Action::Remove => {
                if !matches!(self.observation, Observation::Recognized { .. }) {
                    return;
                }
                RawRegistration::Absent
            }
        };
        let _guard = match self.backend.lock() {
            Ok(guard) => guard,
            Err(error) => {
                self.message = format!("未执行登记操作：{error}");
                return;
            }
        };
        let before = self.backend.read();
        if before.as_ref().ok() != self.snapshot.as_ref() || before.is_err() {
            self.observe(before);
            self.message = "登记已由外部改变或无法读取，本次未修改；请检查新状态。".into();
            return;
        }
        let result = self.backend.write(&desired);
        let after = self.backend.read();
        let unchanged = after.as_ref().ok() == before.as_ref().ok();
        let matches = after.as_ref().ok() == Some(&desired);
        if after.is_err() {
            // Keep the last known target and raw snapshot for recovery context.
            self.unconfirmed = true;
            self.message = format!(
                "结果未确认（{}）：{}；请先刷新，无自动重试或回滚。",
                if action == Action::Remove {
                    "移除登记"
                } else {
                    "登记当前副本"
                },
                after.err().unwrap_or_default()
            );
            self.revision = self.revision.wrapping_add(1);
            return;
        }
        self.observe(after);
        self.message = if matches {
            if action == Action::Remove {
                "登记已移除。"
            } else {
                "已登记并读回确认；Windows 实际允许状态无法验证。"
            }
            .into()
        } else if result.is_err() && unchanged {
            format!(
                "操作失败；已读回原登记：{}",
                result.err().unwrap_or_default()
            )
        } else {
            self.unconfirmed = true;
            format!(
                "结果未确认（{}）：读回内容与请求不符；请先刷新，无自动重试或回滚。",
                if action == Action::Remove {
                    "移除登记"
                } else {
                    "登记当前副本"
                }
            )
        };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn target(storage: &str) -> Target {
        Target {
            executable: r"C:\PathWarp\PathWarp.exe".into(),
            storage: storage.into(),
        }
    }

    #[test]
    fn canonical_commands_preserve_unicode_spaces_and_final_backslashes() {
        for storage in [r"C:\我的 配置\", r"C:\", r"\\server\share\设置"] {
            let expected = target(storage);
            let command = expected.command().unwrap();
            assert_eq!(recognize(&command, Some(&expected)), Some(expected));
        }
        assert!(recognize(r#""C:\Other.exe" --storage-dir "C:\store""#, None).is_none());
        assert!(
            recognize(
                r#""C:\PathWarp.exe" --storage-dir "C:\store" --extra"#,
                None
            )
            .is_none()
        );
    }

    #[test]
    fn uncertain_paths_and_full_command_budget_are_rejected() {
        for storage in [r"C:store", r"\store", r"relative", r"\\?\C:\store"] {
            assert!(target(storage).command().is_err());
        }
        assert!(
            target(&format!("C:\\{}", "a".repeat(250)))
                .command()
                .is_err()
        );
        assert!(target(r"C:\store").command().is_ok());
    }

    struct Fake {
        raw: RefCell<RawRegistration>,
        writes: Cell<usize>,
        fail_write: Cell<bool>,
        fail_read_after: Cell<Option<usize>>,
    }
    impl Backend for Fake {
        fn validate_target(&self, _: &Target) -> Result<(), String> {
            Ok(())
        }
        fn lock(&self) -> Result<Box<dyn std::any::Any>, String> {
            Ok(Box::new(()))
        }
        fn read(&self) -> Result<RawRegistration, String> {
            if self
                .fail_read_after
                .get()
                .is_some_and(|count| self.writes.get() >= count)
            {
                Err("read denied".into())
            } else {
                Ok(self.raw.borrow().clone())
            }
        }
        fn write(&self, value: &RawRegistration) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            if self.fail_write.get() {
                Err("write denied".into())
            } else {
                *self.raw.borrow_mut() = value.clone();
                Ok(())
            }
        }
    }
    fn registration(raw: RawRegistration) -> Registration<Fake> {
        Registration::new(
            Fake {
                raw: RefCell::new(raw),
                writes: Cell::new(0),
                fail_write: Cell::new(false),
                fail_read_after: Cell::new(None),
            },
            Ok(target(r"C:\store")),
        )
    }

    #[test]
    fn launch_only_reads_and_explicit_actions_use_one_registration() {
        let mut registration = registration(RawRegistration::Absent);
        assert_eq!(registration.backend.writes.get(), 0);
        registration.execute(Action::UseCurrent, registration.revision);
        assert!(matches!(
            registration.observation,
            Observation::Recognized { current: true, .. }
        ));
        registration.execute(Action::Remove, registration.revision);
        assert_eq!(registration.observation, Observation::Absent);
        assert_eq!(registration.backend.writes.get(), 2);
    }

    #[test]
    fn another_configuration_is_other_copy_and_unknown_values_are_read_only() {
        let other = target(r"C:\other");
        let registration = registration(RawRegistration::command(&other.command().unwrap()));
        assert!(matches!(
            registration.observation,
            Observation::Recognized { current: false, .. }
        ));
        for raw in [
            RawRegistration::command("unknown.exe"),
            RawRegistration::Value {
                kind: 2,
                bytes: vec![],
            },
        ] {
            let mut registration = self::registration(raw.clone());
            registration.execute(Action::UseCurrent, registration.revision);
            registration.execute(Action::Remove, registration.revision);
            assert_eq!(*registration.backend.raw.borrow(), raw);
            assert_eq!(registration.backend.writes.get(), 0);
        }
    }

    #[test]
    fn external_changes_are_observed_instead_of_overwritten() {
        let mut registration = registration(RawRegistration::Absent);
        *registration.backend.raw.borrow_mut() = RawRegistration::command("foreign");
        registration.execute(Action::UseCurrent, registration.revision);
        assert!(matches!(registration.observation, Observation::Conflict(_)));
        assert_eq!(registration.backend.writes.get(), 0);
    }

    #[test]
    fn stale_clicks_and_unreadable_snapshots_do_not_mutate_and_remove_failures_are_reported() {
        let current = target(r"C:\store");
        let mut registration = registration(RawRegistration::command(&current.command().unwrap()));
        let old_revision = registration.revision;
        registration.refresh();
        registration.execute(Action::Remove, old_revision);
        assert_eq!(registration.backend.writes.get(), 0);
        registration.backend.fail_write.set(true);
        registration.execute(Action::Remove, registration.revision);
        assert!(registration.message.contains("操作失败"));
        assert!(matches!(
            registration.observation,
            Observation::Recognized { .. }
        ));
        registration.backend.fail_read_after.set(Some(0));
        registration.refresh();
        assert!(matches!(
            registration.observation,
            Observation::Unreadable(_)
        ));
        let writes = registration.backend.writes.get();
        registration.execute(Action::Remove, registration.revision);
        assert_eq!(registration.backend.writes.get(), writes);
        assert_eq!(registration.last_target, Some(current));
    }

    #[test]
    fn failed_write_is_distinct_from_unconfirmed_readback_and_never_retried() {
        let mut registration = registration(RawRegistration::Absent);
        registration.backend.fail_write.set(true);
        registration.execute(Action::UseCurrent, registration.revision);
        assert!(!registration.unconfirmed);
        assert!(registration.message.contains("操作失败"));
        registration.backend.fail_write.set(false);
        registration
            .backend
            .fail_read_after
            .set(Some(registration.backend.writes.get() + 1));
        registration.execute(Action::UseCurrent, registration.revision);
        assert!(registration.unconfirmed);
        let writes = registration.backend.writes.get();
        registration.execute(Action::Remove, registration.revision);
        assert_eq!(registration.backend.writes.get(), writes);
        registration.backend.fail_read_after.set(None);
        registration.refresh();
        assert!(!registration.unconfirmed);
        assert!(matches!(
            registration.observation,
            Observation::Recognized { .. }
        ));
    }
}
