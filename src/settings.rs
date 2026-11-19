//! Presentation shared by native settings controls and the application command
//! queue. The native surface never owns another copy of configuration state.
use crate::config::{ConfigStore, SaveState, ThemePreference};
use crate::startup::{Backend, Observation, Registration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub theme: ThemePreference,
    pub theme_status: String,
    pub retry_save: bool,
    pub startup_status: String,
    pub use_label: String,
    pub can_use: bool,
    pub can_remove: bool,
    pub revision: u64,
}

impl View {
    pub fn new<B: Backend>(
        theme: ThemePreference,
        config: &ConfigStore,
        registration: &Registration<B>,
    ) -> Self {
        let mut theme_status = match config.save_state() {
            SaveState::Unchanged => "主题选择后立即生效并自动保存。".into(),
            SaveState::Saved => "主题已保存。".into(),
            SaveState::Unsaved(reason) => format!("未保存，仅本次生效。\r\n{reason}"),
        };
        if let Some(diagnostic) = config.load_diagnostic() {
            theme_status.push_str(&format!("\r\n启动时加载提示：{diagnostic}"));
        }
        if let Some(directory) = config.storage_directory() {
            theme_status.push_str(&format!("\r\n配置目录：{}", directory.display()));
        }
        let startup_status = match &registration.observation {
            Observation::Absent => "未登记。".into(),
            Observation::Recognized {
                target,
                current,
                exists,
            } => format!(
                "已登记：{}{}\r\n程序：{}\r\n配置目录：{}",
                if *current {
                    "当前副本"
                } else {
                    "其他副本／配置"
                },
                if *exists {
                    ""
                } else {
                    "（目标已失效或不可访问）"
                },
                target.executable.display(),
                target.storage.display()
            ),
            Observation::Conflict(reason) => format!("登记冲突：{reason}"),
            Observation::Unreadable(reason) => format!("无法读取登记：{reason}"),
        };
        let mut startup_status = format!(
            "{startup_status}\r\nWindows 实际允许状态：无法验证。可在 Windows 设置／任务管理器确认。\r\n{}",
            registration.message
        );
        if registration.unconfirmed
            && !matches!(registration.observation, Observation::Recognized { .. })
            && let Some(target) = &registration.last_target
        {
            startup_status.push_str(&format!(
                "\r\n最后已知程序：{}\r\n最后已知配置目录：{}",
                target.executable.display(),
                target.storage.display()
            ));
        }
        if let Err(error) = &registration.target {
            startup_status.push_str(&format!("\r\n当前副本不可登记：{error}"));
        }
        let writable = registration.observation.writable() && !registration.unconfirmed;
        Self {
            theme,
            theme_status,
            retry_save: matches!(config.save_state(), SaveState::Unsaved(_))
                || (config.load_diagnostic().is_some()
                    && !matches!(config.save_state(), SaveState::Saved)),
            startup_status,
            use_label: if matches!(registration.observation, Observation::Absent) {
                "添加此副本的登录启动项"
            } else {
                "改用当前副本（共享登记）"
            }
            .into(),
            can_use: writable
                && registration.target.is_ok()
                && !matches!(
                    registration.observation,
                    Observation::Recognized {
                        current: true,
                        exists: true,
                        ..
                    }
                ),
            can_remove: writable
                && matches!(registration.observation, Observation::Recognized { .. }),
            revision: registration.revision,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::startup::{RawRegistration, Target};
    struct ReadOnly(RawRegistration);
    impl Backend for ReadOnly {
        fn lock(&self) -> Result<Box<dyn std::any::Any>, String> {
            Err("read-only fixture".into())
        }
        fn read(&self) -> Result<RawRegistration, String> {
            Ok(self.0.clone())
        }
        fn write(&self, _: &RawRegistration) -> Result<(), String> {
            panic!("view must never write registration");
        }
        fn validate_target(&self, _: &Target) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn view_exposes_save_retry_other_target_and_read_only_conflicts_independently() {
        let (config, _) = ConfigStore::open(None);
        let current = Target {
            executable: r"C:\PathWarp.exe".into(),
            storage: r"C:\current".into(),
        };
        let other = Target {
            storage: r"C:\其他配置".into(),
            ..current.clone()
        };
        let registration = Registration::new(
            ReadOnly(RawRegistration::command(&other.command().unwrap())),
            Ok(current.clone()),
        );
        let view = View::new(ThemePreference::Dark, &config, &registration);
        assert!(view.retry_save);
        assert!(view.theme_status.contains("未保存，仅本次生效"));
        assert!(view.startup_status.contains("其他配置"));
        assert!(
            view.startup_status
                .contains("Windows 实际允许状态：无法验证")
        );
        assert!(view.can_use && view.can_remove);
        let conflict = Registration::new(
            ReadOnly(RawRegistration::command("unknown.exe")),
            Ok(current),
        );
        let view = View::new(ThemePreference::Light, &config, &conflict);
        assert!(!view.can_use && !view.can_remove);
        assert!(
            view.retry_save,
            "a registry conflict must not disable theme retry"
        );
    }
}
