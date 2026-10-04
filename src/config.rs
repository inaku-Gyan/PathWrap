//! User-facing configuration shared by the application shell and settings UI.
//!
//! The configuration file is deliberately small.  Theme selection is kept out of
//! the renderer so a future settings surface can update and persist the same model
//! without introducing another source of truth.

use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_STORAGE_DIRECTORY: &str = "pathwrap-store";
const CONFIG_FILE: &str = "config.json";
const STORAGE_DIRECTORY_ARGUMENT: &str = "--storage-dir";

/// Errors produced while parsing the explicit storage-directory override.
#[derive(Debug, PartialEq, Eq)]
pub enum StorageArgsError {
    UnknownArgument(OsString),
    MissingValue,
    EmptyValue,
    DuplicateStorageDirectory,
    ExecutableDirectoryUnavailable,
}

impl fmt::Display for StorageArgsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(argument) => {
                write!(formatter, "unknown command-line argument {:?}", argument)
            }
            Self::MissingValue => write!(formatter, "--storage-dir requires a path value"),
            Self::EmptyValue => write!(formatter, "--storage-dir requires a non-empty path"),
            Self::DuplicateStorageDirectory => {
                write!(formatter, "--storage-dir may be specified only once")
            }
            Self::ExecutableDirectoryUnavailable => {
                write!(formatter, "the executable directory is unavailable")
            }
        }
    }
}

impl std::error::Error for StorageArgsError {}

/// Parse the optional storage-directory override without needing the executable
/// location. This lets startup reject malformed arguments even if Windows cannot
/// resolve `current_exe()`.
pub fn parse_storage_dir<I, S>(args: I) -> Result<Option<PathBuf>, StorageArgsError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut selected = None;
    let mut arguments = args.into_iter().map(Into::into);
    while let Some(argument) = arguments.next() {
        if argument != OsStr::new(STORAGE_DIRECTORY_ARGUMENT) {
            return Err(StorageArgsError::UnknownArgument(argument));
        }
        if selected.is_some() {
            return Err(StorageArgsError::DuplicateStorageDirectory);
        }

        let Some(value) = arguments.next() else {
            return Err(StorageArgsError::MissingValue);
        };
        if value.is_empty() {
            return Err(StorageArgsError::EmptyValue);
        }
        if value == OsStr::new(STORAGE_DIRECTORY_ARGUMENT) {
            return Err(StorageArgsError::DuplicateStorageDirectory);
        }
        selected = Some(PathBuf::from(value));
    }

    Ok(selected)
}

pub fn resolve_storage_path_for_process(
    selected: Option<PathBuf>,
    executable: &Path,
) -> Result<PathBuf, StorageArgsError> {
    let executable_directory = executable
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(StorageArgsError::ExecutableDirectoryUnavailable)?;

    let selected = selected.unwrap_or_else(|| PathBuf::from(DEFAULT_STORAGE_DIRECTORY));
    Ok(if selected.is_absolute() {
        selected
    } else {
        executable_directory.join(selected)
    })
}

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
#[serde(deny_unknown_fields)]
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
    /// Load from an explicit path.  This is also the deterministic seam used by
    /// tests and by callers that need to inspect a candidate configuration.
    #[allow(dead_code)]
    pub fn load_from(path: &Path) -> Self {
        match read_config_file(path) {
            Ok(Some(config)) => config,
            Ok(None) => Self::default(),
            Err(error) => {
                log::warn!(
                    "could not read configuration {}: {error}; using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Persist to an explicit path using the same atomic write as [`ConfigStore`].
    #[allow(dead_code)]
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_config_atomic(path, self)
    }
}

/// Owns the selected configuration path and the process-local persistence mode.
///
/// Once a storage operation fails, the store remains memory-only until the next
/// launch. This keeps the UI responsive without repeatedly warning on every
/// theme change or risking a partial configuration write.
#[derive(Debug)]
pub struct ConfigStore {
    config_path: Option<PathBuf>,
    memory_only: bool,
}

impl ConfigStore {
    /// Open the selected storage directory and load its configuration.
    pub fn open(storage_directory: Option<PathBuf>) -> (Self, AppConfig) {
        let Some(storage_directory) = storage_directory else {
            log::warn!("configuration storage is unavailable; running in memory-only mode");
            return (Self::memory_only(None), AppConfig::default());
        };

        if let Err(error) = fs::create_dir_all(&storage_directory) {
            log::warn!(
                "could not create configuration storage {}: {error}; running in memory-only mode",
                storage_directory.display()
            );
            return (
                Self::memory_only(Some(storage_directory.join(CONFIG_FILE))),
                AppConfig::default(),
            );
        }

        let config_path = storage_directory.join(CONFIG_FILE);
        let config = match read_config_file(&config_path) {
            Ok(Some(config)) => config,
            Ok(None) => AppConfig::default(),
            Err(error) => {
                log::warn!(
                    "could not access configuration {}: {error}; running in memory-only mode",
                    config_path.display()
                );
                return (Self::memory_only(Some(config_path)), AppConfig::default());
            }
        };

        (
            Self {
                config_path: Some(config_path),
                memory_only: false,
            },
            config,
        )
    }

    fn memory_only(config_path: Option<PathBuf>) -> Self {
        Self {
            config_path,
            memory_only: true,
        }
    }

    /// Whether this process has stopped attempting persistence.
    #[allow(dead_code)]
    pub fn is_memory_only(&self) -> bool {
        self.memory_only
    }

    /// Save the current configuration, disabling persistence after the first
    /// failure while retaining the caller's in-memory state.
    pub fn save(&mut self, config: &AppConfig) {
        if self.memory_only {
            return;
        }

        let Some(config_path) = self.config_path.as_deref() else {
            self.memory_only = true;
            return;
        };
        if let Err(error) = write_config_atomic(config_path, config) {
            log::warn!(
                "could not persist configuration {}: {error}; switching to memory-only mode",
                config_path.display()
            );
            self.memory_only = true;
        }
    }
}

fn read_config_file(path: &Path) -> io::Result<Option<AppConfig>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    match serde_json::from_str(&contents) {
        Ok(config) => Ok(Some(config)),
        Err(error) => {
            log::warn!(
                "could not parse configuration {}: {error}; using defaults",
                path.display()
            );
            Ok(Some(AppConfig::default()))
        }
    }
}

fn write_config_atomic(path: &Path, config: &AppConfig) -> io::Result<()> {
    let encoded = serde_json::to_vec_pretty(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let temporary_path = path.with_file_name(format!("{CONFIG_FILE}.tmp"));

    let write_result = (|| {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)?;
        io::Write::write_all(&mut file, &encoded)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary_path, path)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

#[cfg(windows)]
fn replace_file(temporary_path: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let temporary = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    unsafe {
        MoveFileExW(
            PCWSTR::from_raw(temporary.as_ptr()),
            PCWSTR::from_raw(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(io::Error::other)
}

#[cfg(not(windows))]
fn replace_file(temporary_path: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary_path, destination)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn executable_path() -> PathBuf {
        PathBuf::from("portable").join("PathWarp.exe")
    }

    fn resolve_storage_dir<I, S>(args: I, executable: &Path) -> Result<PathBuf, StorageArgsError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let selected = parse_storage_dir(args)?;
        resolve_storage_path_for_process(selected, executable)
    }

    #[test]
    fn storage_directory_defaults_beside_executable() {
        assert_eq!(
            resolve_storage_dir(Vec::<OsString>::new(), &executable_path())
                .expect("default storage path should resolve"),
            PathBuf::from("portable").join("pathwrap-store")
        );
    }

    #[test]
    fn storage_directory_accepts_relative_and_absolute_overrides() {
        assert_eq!(
            resolve_storage_dir(
                [
                    OsString::from("--storage-dir"),
                    OsString::from("saved settings")
                ],
                &executable_path()
            )
            .expect("relative override should resolve"),
            PathBuf::from("portable").join("saved settings")
        );

        let absolute = std::env::temp_dir().join("PathWarp-explicit-storage");
        assert_eq!(
            resolve_storage_dir(
                [
                    OsString::from("--storage-dir"),
                    absolute.clone().into_os_string()
                ],
                &executable_path()
            )
            .expect("absolute override should be accepted"),
            absolute
        );
    }

    #[test]
    fn storage_directory_accepts_names_starting_with_dashes() {
        assert_eq!(
            resolve_storage_dir(
                [OsString::from("--storage-dir"), OsString::from("--cache")],
                &executable_path()
            )
            .expect("a path beginning with dashes should be accepted"),
            PathBuf::from("portable").join("--cache")
        );
    }

    #[test]
    fn storage_directory_rejects_malformed_arguments() {
        let executable = executable_path();
        assert_eq!(
            resolve_storage_dir([OsString::from("--unknown")], &executable),
            Err(StorageArgsError::UnknownArgument(OsString::from(
                "--unknown"
            )))
        );
        assert_eq!(
            resolve_storage_dir([OsString::from("--storage-dir")], &executable),
            Err(StorageArgsError::MissingValue)
        );
        assert_eq!(
            resolve_storage_dir(
                [OsString::from("--storage-dir"), OsString::new()],
                &executable
            ),
            Err(StorageArgsError::EmptyValue)
        );
        assert_eq!(
            resolve_storage_dir(
                [
                    OsString::from("--storage-dir"),
                    OsString::from("one"),
                    OsString::from("--storage-dir"),
                    OsString::from("two"),
                ],
                &executable
            ),
            Err(StorageArgsError::DuplicateStorageDirectory)
        );
        assert_eq!(
            resolve_storage_dir(
                [
                    OsString::from("--storage-dir"),
                    OsString::from("--storage-dir"),
                ],
                &executable
            ),
            Err(StorageArgsError::DuplicateStorageDirectory)
        );
    }

    fn temporary_path(label: &str) -> PathBuf {
        temporary_directory(label).join("config.json")
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("PathWarp-{label}-{}-{id}", std::process::id()))
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
    fn unknown_config_fields_fall_back_to_defaults() {
        let path = temporary_path("unknown-field");
        fs::create_dir_all(path.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&path, r#"{"theme":"dark","window":"large"}"#)
            .expect("unknown config should be writable");
        assert_eq!(AppConfig::load_from(&path), AppConfig::default());
        let _ = fs::remove_dir_all(path.parent().expect("temporary path has parent"));
    }

    #[test]
    fn missing_theme_field_uses_default() {
        let path = temporary_path("missing-theme");
        fs::create_dir_all(path.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&path, b"{}").expect("config should be writable");
        assert_eq!(AppConfig::load_from(&path), AppConfig::default());
        let _ = fs::remove_dir_all(path.parent().expect("temporary path has parent"));
    }

    #[test]
    fn empty_config_falls_back_to_defaults() {
        let path = temporary_path("empty-config");
        fs::create_dir_all(path.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&path, b"").expect("config should be writable");
        assert_eq!(AppConfig::load_from(&path), AppConfig::default());
        let _ = fs::remove_dir_all(path.parent().expect("temporary path has parent"));
    }

    #[test]
    fn invalid_theme_type_falls_back_to_defaults() {
        let path = temporary_path("invalid-theme-type");
        fs::create_dir_all(path.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&path, br#"{"theme":{}}"#).expect("config should be writable");
        assert_eq!(AppConfig::load_from(&path), AppConfig::default());
        let _ = fs::remove_dir_all(path.parent().expect("temporary path has parent"));
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

    #[test]
    fn config_store_creates_storage_and_round_trips_config() {
        let storage = temporary_directory("store-round-trip");
        let (mut store, loaded) = ConfigStore::open(Some(storage.clone()));
        assert_eq!(loaded, AppConfig::default());
        assert!(!store.is_memory_only());

        let expected = AppConfig {
            theme: ThemePreference::Dark,
        };
        store.save(&expected);
        assert!(!store.is_memory_only());
        assert_eq!(AppConfig::load_from(&storage.join(CONFIG_FILE)), expected);
        let _ = fs::remove_dir_all(storage);
    }

    #[test]
    fn config_store_ignores_stale_temporary_file() {
        let storage = temporary_directory("store-stale-temp");
        fs::create_dir_all(&storage).expect("storage directory should be creatable");
        let expected = AppConfig {
            theme: ThemePreference::Dark,
        };
        expected
            .save_to(&storage.join(CONFIG_FILE))
            .expect("config should save");
        let stale_temporary = storage.join(format!("{CONFIG_FILE}.tmp"));
        fs::write(&stale_temporary, br#"{"theme":"light"}"#)
            .expect("stale temp config should be writable");
        assert!(stale_temporary.exists());

        let (store, loaded) = ConfigStore::open(Some(storage.clone()));
        assert!(!store.is_memory_only());
        assert_eq!(loaded, expected);
        let _ = fs::remove_dir_all(storage);
    }

    #[test]
    fn config_store_enters_memory_only_mode_when_storage_cannot_be_created() {
        let blocker = temporary_path("storage-blocker");
        fs::create_dir_all(blocker.parent().expect("temporary path has parent"))
            .expect("temporary directory should be creatable");
        fs::write(&blocker, b"not a directory").expect("storage blocker should be writable");

        let (store, loaded) = ConfigStore::open(Some(blocker.join("pathwrap-store")));
        assert!(store.is_memory_only());
        assert_eq!(loaded, AppConfig::default());
        let _ = fs::remove_dir_all(blocker.parent().expect("temporary path has parent"));
    }

    #[test]
    fn config_store_disables_persistence_after_atomic_replace_failure() {
        let storage = temporary_directory("store-save-failure");
        let (mut store, _) = ConfigStore::open(Some(storage.clone()));
        fs::create_dir(storage.join(CONFIG_FILE))
            .expect("config destination should be a directory");

        let expected = AppConfig {
            theme: ThemePreference::Light,
        };
        store.save(&expected);
        assert!(store.is_memory_only());
        assert!(storage.join(CONFIG_FILE).is_dir());
        assert!(!storage.join(format!("{CONFIG_FILE}.tmp")).exists());

        store.save(&AppConfig {
            theme: ThemePreference::Dark,
        });
        assert!(store.is_memory_only());
        assert!(!storage.join(format!("{CONFIG_FILE}.tmp")).exists());
        let _ = fs::remove_dir_all(storage);
    }
}
