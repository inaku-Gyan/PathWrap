//! User-facing configuration shared by the application shell and settings UI.
//!
//! The configuration file is deliberately small.  Theme selection is kept out of
//! the renderer so a future settings surface can update and persist the same model
//! without introducing another source of truth.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const APP_DIRECTORY: &str = "PathWarp";
const CONFIG_FILE: &str = "config.json";

/// The user's requested theme behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Follow the Windows application theme.
    #[default]
    Auto,
    /// Always use the dark palette.
    Dark,
    /// Always use the light palette.
    Light,
}

impl ThemePreference {
    /// Stable user-facing label used by the compact theme menu and assistive text.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "跟随系统",
            Self::Dark => "深色",
            Self::Light => "浅色",
        }
    }
}

/// A concrete palette mode after resolving [`ThemePreference::Auto`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    /// Stable label for the currently resolved visual mode.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "深色",
            Self::Light => "浅色",
        }
    }
}

impl ThemePreference {
    /// Resolve this preference against the currently detected system mode.
    pub const fn resolve(self, system_mode: ThemeMode) -> ThemeMode {
        match self {
            Self::Auto => system_mode,
            Self::Dark => ThemeMode::Dark,
            Self::Light => ThemeMode::Light,
        }
    }
}

/// Persisted application settings.  More settings can be added here without
/// making the theme renderer responsible for file I/O.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub theme: ThemePreference,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Auto,
        }
    }
}

impl AppConfig {
    /// Load the current user's configuration, falling back to defaults when the
    /// directory, file, or contents cannot be read.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            log::debug!("configuration directory is unavailable; using defaults");
            return Self::default();
        };
        Self::load_from(&path)
    }

    /// Load from an explicit path.  This is also the deterministic seam used by
    /// tests and by callers that need to inspect a candidate configuration.
    pub fn load_from(path: &Path) -> Self {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) => {
                log::debug!(
                    "could not read configuration {}: {error}; using defaults",
                    path.display()
                );
                return Self::default();
            }
        };

        match serde_json::from_str(&contents) {
            Ok(config) => config,
            Err(error) => {
                log::warn!(
                    "could not parse configuration {}: {error}; using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Persist the current configuration in the user's configuration directory.
    #[allow(dead_code)] // Used by the settings UI introduced in issue #30.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = config_path() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "user configuration directory is unavailable",
            ));
        };
        self.save_to(&path)
    }

    /// Persist to an explicit path, creating its parent directory when needed.
    #[allow(dead_code)] // Also serves as the persistence seam for issue #30.
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encoded = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(path, encoded)
    }
}

/// Resolve the per-user configuration path used by the Windows application.
///
/// `%APPDATA%` is the normal Windows roaming configuration root.  The
/// `%LOCALAPPDATA%` fallback keeps the app usable in restricted environments
/// where the roaming variable is not present.
pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(|root| PathBuf::from(root).join(APP_DIRECTORY).join(CONFIG_FILE))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("PathWarp-{label}-{nonce}"))
            .join("config.json")
    }

    #[test]
    fn default_theme_follows_system() {
        assert_eq!(AppConfig::default().theme, ThemePreference::Auto);
        assert_eq!(
            ThemePreference::Auto.resolve(ThemeMode::Dark),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::Auto.resolve(ThemeMode::Light),
            ThemeMode::Light
        );
    }

    #[test]
    fn explicit_theme_overrides_system() {
        assert_eq!(
            ThemePreference::Dark.resolve(ThemeMode::Light),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::Light.resolve(ThemeMode::Dark),
            ThemeMode::Light
        );
    }

    #[test]
    fn theme_preference_serializes_with_stable_values() {
        let config = AppConfig {
            theme: ThemePreference::Light,
        };
        let encoded = serde_json::to_string(&config).expect("config should serialize");
        assert_eq!(encoded, r#"{"theme":"light"}"#);
        assert_eq!(
            serde_json::from_str::<AppConfig>(&encoded).expect("config should deserialize"),
            config
        );
    }

    #[test]
    fn missing_or_invalid_config_falls_back_to_auto() {
        let missing = temporary_path("missing");
        assert_eq!(AppConfig::load_from(&missing), AppConfig::default());

        let invalid = temporary_path("invalid");
        fs::create_dir_all(invalid.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&invalid, r#"{"theme":"sepia"}"#).expect("invalid config should be writable");
        assert_eq!(AppConfig::load_from(&invalid), AppConfig::default());
        let _ = fs::remove_dir_all(invalid.parent().expect("temporary path has parent"));
    }

    #[test]
    fn config_round_trip_persists_theme() {
        let path = temporary_path("round-trip");
        let expected = AppConfig {
            theme: ThemePreference::Dark,
        };
        expected.save_to(&path).expect("config should save");
        assert_eq!(AppConfig::load_from(&path), expected);
        let _ = fs::remove_dir_all(path.parent().expect("temporary path has parent"));
    }
}
