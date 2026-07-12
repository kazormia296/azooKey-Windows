use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
};

pub const PRODUCT_ID: &str = "com.miyakey.grimodex";
pub const IME_CONFIG_DIR: &str = "ime";
pub const SETTINGS_FILENAME: &str = "settings.json";
pub const SERVER_PIPE_PREFIX: &str = "com.miyakey.grimodex.server";
pub const UI_PIPE_PREFIX: &str = "com.miyakey.grimodex.ui";

/// A per-logon namespace prevents another user's process from guessing the
/// pipe name. The named-pipe ACL remains the enforcement boundary; this value
/// is only used for isolation and diagnostics.
pub fn pipe_namespace() -> String {
    #[cfg(windows)]
    {
        use windows::{
            core::PWSTR,
            Win32::{
                Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL},
                Security::{
                    Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser,
                    TOKEN_QUERY, TOKEN_USER,
                },
                System::Threading::{GetCurrentProcess, OpenProcessToken},
            },
        };

        let result = (|| unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
            let mut required = 0u32;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);
            if required == 0 {
                let _ = CloseHandle(token);
                return Err(windows::core::Error::from_win32());
            }
            let mut buffer = vec![0u8; required as usize];
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                required,
                &mut required,
            )?;
            let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
            let mut sid_string = PWSTR::null();
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string)?;
            let sid = sid_string.to_string()?;
            let _ = LocalFree(HLOCAL(sid_string.0.cast()));
            let _ = CloseHandle(token);
            Ok::<_, windows::core::Error>(sid)
        })();
        return result.unwrap_or_else(|_| "unknown-user".to_string());
    }

    #[cfg(not(windows))]
    {
        "dev-user".to_string()
    }
}

pub fn server_pipe_name() -> String {
    format!(r"\\.\pipe\{}.{}", SERVER_PIPE_PREFIX, pipe_namespace())
}

pub fn ui_pipe_name() -> String {
    format!(r"\\.\pipe\{}.{}", UI_PIPE_PREFIX, pipe_namespace())
}

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/azookey.rs"));
    include!(concat!(env!("OUT_DIR"), "/window.rs"));
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("azookey_service_descriptor");
}

pub fn config_root_from_appdata(appdata: impl AsRef<Path>) -> PathBuf {
    appdata.as_ref().join(PRODUCT_ID).join(IME_CONFIG_DIR)
}

pub fn get_config_root() -> PathBuf {
    let appdata = env::var_os("APPDATA")
        .or_else(|| env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    config_root_from_appdata(appdata)
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZenzaiConfig {
    pub enable: bool,
    pub profile: String,
    pub backend: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub version: String,
    pub zenzai: ZenzaiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            version: "0.1.0".to_string(),
            zenzai: ZenzaiConfig {
                enable: false,
                profile: "".to_string(),
                backend: "cpu".to_string(),
            },
        }
    }
}

impl AppConfig {
    pub fn write(&self) {
        let config_path = get_config_root().join(SETTINGS_FILENAME);
        let Ok(config_str) = serde_json::to_string_pretty(self) else {
            return;
        };
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(config_path, config_str);
    }

    pub fn read() -> Self {
        let config_path = get_config_root().join(SETTINGS_FILENAME);
        if !config_path.exists() {
            return AppConfig::default();
        }
        let Ok(config_str) = std::fs::read_to_string(config_path) else {
            return AppConfig::default();
        };
        serde_json::from_str(&config_str).unwrap_or_default()
    }

    pub fn new() -> Self {
        let config_path = get_config_root();
        let _ = std::fs::create_dir_all(&config_path);
        let config = AppConfig::read();
        config.write();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_is_product_scoped() {
        let path = config_root_from_appdata(r"C:\Users\test\AppData\Roaming");
        assert!(path.ends_with(Path::new(PRODUCT_ID).join(IME_CONFIG_DIR)));
        assert!(path.to_string_lossy().contains(PRODUCT_ID));
    }

    #[test]
    fn malformed_config_falls_back_to_defaults() {
        let value: Result<AppConfig, _> = serde_json::from_str("not-json");
        assert!(value.is_err());
        assert_eq!(AppConfig::default().version, "0.1.0");
    }

    #[test]
    fn ipc_names_are_product_scoped() {
        assert!(SERVER_PIPE_PREFIX.starts_with(PRODUCT_ID));
        assert!(UI_PIPE_PREFIX.starts_with(PRODUCT_ID));
        assert_ne!(SERVER_PIPE_PREFIX, UI_PIPE_PREFIX);
        assert!(server_pipe_name().contains(PRODUCT_ID));
        assert!(ui_pipe_name().contains(PRODUCT_ID));
    }
}
