//! The only production registry location owned by PathWarp. Tests instantiate
//! the same adapter under a disposable non-Run key; no real logon mutation.
use crate::startup::{Backend, RawRegistration, Target};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, HANDLE, HLOCAL, LocalFree, WAIT_ABANDONED,
    WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
    REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, OpenProcessToken, ReleaseMutex, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "inaku-Gyan.PathWarp";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct ActionLock(Handle);
impl Drop for ActionLock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0.0);
        }
    }
}

fn user_sid() -> Result<String, String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| e.to_string())?;
        let token = Handle(token);
        let mut size = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
        if size == 0 || size > 65536 {
            return Err("无法读取当前用户身份。".into());
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .map_err(|e| e.to_string())?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid).map_err(|e| e.to_string())?;
        let result = sid.to_string().map_err(|e| e.to_string());
        let _ = LocalFree(Some(HLOCAL(sid.0.cast())));
        result
    }
}

pub struct RunRegistration {
    key: String,
    name: String,
}
impl Default for RunRegistration {
    fn default() -> Self {
        Self {
            key: RUN_KEY.into(),
            name: VALUE_NAME.into(),
        }
    }
}

impl Backend for RunRegistration {
    fn validate_target(&self, target: &Target) -> Result<(), String> {
        Target::current(&target.executable, &target.storage).map(|_| ())
    }

    fn lock(&self) -> Result<Box<dyn std::any::Any>, String> {
        let name = wide(&format!(
            r"Global\inaku-Gyan.PathWarp.LoginStartup.{}",
            user_sid()?
        ));
        let handle = Handle(
            unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }
                .map_err(|e| e.to_string())?,
        );
        let result = unsafe { WaitForSingleObject(handle.0, 1000) };
        if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
            return Err("另一个副本正在修改登录启动项，请刷新后再试。".into());
        }
        Ok(Box::new(ActionLock(handle)))
    }

    fn read(&self) -> Result<RawRegistration, String> {
        let path = wide(&self.key);
        let name = wide(&self.name);
        let mut key = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                None,
                KEY_QUERY_VALUE,
                &mut key,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(RawRegistration::Absent);
        }
        status.ok().map_err(|e| e.to_string())?;
        let key = Key(key);
        for _ in 0..3 {
            let mut size = 0;
            let mut kind = REG_VALUE_TYPE::default();
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    None,
                    Some(&mut size),
                )
            };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(RawRegistration::Absent);
            }
            status.ok().map_err(|e| e.to_string())?;
            if size > 65536 {
                return Err("同名登记过大，无法安全读取；保持只读。".into());
            }
            let mut bytes = vec![0u8; size as usize];
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(bytes.as_mut_ptr()),
                    Some(&mut size),
                )
            };
            if status == ERROR_MORE_DATA {
                continue;
            }
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(RawRegistration::Absent);
            }
            status.ok().map_err(|e| e.to_string())?;
            bytes.truncate(size as usize);
            return Ok(RawRegistration::Value {
                kind: kind.0,
                bytes,
            });
        }
        Err("登记连续发生变化，请刷新。".into())
    }

    fn write(&self, value: &RawRegistration) -> Result<(), String> {
        let path = wide(&self.key);
        let name = wide(&self.name);
        let mut key = HKEY::default();
        let status = match value {
            RawRegistration::Absent => unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    None,
                    KEY_SET_VALUE,
                    &mut key,
                )
            },
            RawRegistration::Value { .. } => unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    None,
                    None,
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    None,
                    &mut key,
                    None,
                )
            },
        };
        if status == ERROR_FILE_NOT_FOUND && matches!(value, RawRegistration::Absent) {
            return Ok(());
        }
        status.ok().map_err(|e| e.to_string())?;
        let key = Key(key);
        let status = match value {
            RawRegistration::Absent => unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) },
            RawRegistration::Value { kind, bytes } => unsafe {
                RegSetValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    REG_VALUE_TYPE(*kind),
                    Some(bytes),
                )
            },
        };
        if status == ERROR_FILE_NOT_FOUND && matches!(value, RawRegistration::Absent) {
            return Ok(());
        }
        status.ok().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::RegDeleteTreeW;

    #[test]
    fn adapter_round_trip_is_confined_to_a_disposable_non_run_key() {
        let backend = RunRegistration {
            key: format!(
                r"Software\PathWarpTests\{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            name: "owned".into(),
        };
        assert!(!backend.key.contains(r"CurrentVersion\Run"));
        let _guard = backend.lock().unwrap();
        assert_eq!(backend.read().unwrap(), RawRegistration::Absent);
        let value =
            RawRegistration::command(r#""C:\中文 程序\PathWarp.exe" --storage-dir "C:\配置\\""#);
        backend.write(&value).unwrap();
        assert_eq!(backend.read().unwrap(), value);
        let unrelated = RunRegistration {
            key: backend.key.clone(),
            name: "unrelated".into(),
        };
        unrelated
            .write(&RawRegistration::command("preserved"))
            .unwrap();
        backend.write(&RawRegistration::Absent).unwrap();
        assert_eq!(backend.read().unwrap(), RawRegistration::Absent);
        assert_eq!(
            unrelated.read().unwrap(),
            RawRegistration::command("preserved")
        );
        let key = wide(&backend.key);
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()))
                .ok()
                .unwrap();
        }
    }
}
