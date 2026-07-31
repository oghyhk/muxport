use crate::jsonrpc::ProcessConfig;
use crate::CodexAdapter;
use adapter_api::AdapterError;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManagedCodexError {
    #[error("invalid managed Codex profile id")]
    InvalidProfileId,
    #[error("managed Codex executable path must be absolute")]
    ExecutableNotAbsolute,
    #[error("managed Codex project directory must be absolute")]
    ProjectDirectoryNotAbsolute,
    #[error("managed Codex profile path contains a symbolic link: {0}")]
    SymbolicLink(PathBuf),
    #[error("managed Codex profile path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("managed Codex profile I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("managed Codex adapter configuration failed: {0}")]
    Adapter(#[from] AdapterError),
}

/// A connector-owned Codex profile with isolated configuration, auth,
/// sessions, logs, skills metadata, and SQLite state.
///
/// App Server remains the sole owner of the profile's credentials. Muxport
/// selects a private `CODEX_HOME`, forces Codex's supported file credential
/// backend for profile separation, and uses only the supported `account/*`
/// JSON-RPC API for login and logout.
#[derive(Clone, Debug)]
pub struct ManagedCodexProfile {
    profile_id: String,
    executable: PathBuf,
    project_directory: PathBuf,
    profile_root: PathBuf,
    codex_home: PathBuf,
    sqlite_home: PathBuf,
}

impl ManagedCodexProfile {
    pub fn prepare(
        profiles_root: impl AsRef<Path>,
        profile_id: impl Into<String>,
        executable: impl Into<PathBuf>,
        project_directory: impl Into<PathBuf>,
    ) -> Result<Self, ManagedCodexError> {
        let profile_id = profile_id.into();
        if !valid_profile_id(&profile_id) {
            return Err(ManagedCodexError::InvalidProfileId);
        }
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(ManagedCodexError::ExecutableNotAbsolute);
        }
        let project_directory = project_directory.into();
        if !project_directory.is_absolute() {
            return Err(ManagedCodexError::ProjectDirectoryNotAbsolute);
        }

        let vendor_root = profiles_root.as_ref().join("codex");
        let profile_root = vendor_root.join(&profile_id);
        let codex_home = profile_root.join("home");
        let sqlite_home = profile_root.join("sqlite");
        let os_home = profile_root.join("os-home");
        for directory in [
            &vendor_root,
            &profile_root,
            &codex_home,
            &sqlite_home,
            &os_home,
        ] {
            create_private_directory(directory)?;
        }

        Ok(Self {
            profile_id,
            executable,
            project_directory,
            profile_root,
            codex_home,
            sqlite_home,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    pub fn adapter(&self) -> CodexAdapter {
        CodexAdapter::new_managed(self.process_config(), self.profile_id.clone())
    }

    fn process_config(&self) -> ProcessConfig {
        let mut environment = vec![
            (
                OsString::from("CODEX_HOME"),
                self.codex_home.clone().into_os_string(),
            ),
            (
                OsString::from("CODEX_SQLITE_HOME"),
                self.sqlite_home.clone().into_os_string(),
            ),
            (
                OsString::from("HOME"),
                self.profile_root.join("os-home").into_os_string(),
            ),
            (
                OsString::from("USERPROFILE"),
                self.profile_root.join("os-home").into_os_string(),
            ),
        ];
        for name in bootstrap_environment_names() {
            if let Some(value) = std::env::var_os(name) {
                environment.push((OsString::from(name), value));
            }
        }
        ProcessConfig::isolated(
            &self.executable,
            &self.project_directory,
            vec![
                OsString::from("-c"),
                OsString::from(r#"cli_auth_credentials_store="file""#),
            ],
            environment,
        )
    }
}

fn valid_profile_id(profile_id: &str) -> bool {
    !profile_id.is_empty()
        && profile_id.len() <= 128
        && profile_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn create_private_directory(path: &Path) -> Result<(), ManagedCodexError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(ManagedCodexError::SymbolicLink(path.to_owned()));
        }
        if !metadata.is_dir() {
            return Err(ManagedCodexError::NotDirectory(path.to_owned()));
        }
    } else {
        fs::create_dir_all(path)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(ManagedCodexError::SymbolicLink(path.to_owned()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn bootstrap_environment_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT"]
    } else {
        &["LANG", "LC_ALL", "TZ"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "muxport-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn prepares_private_codex_and_sqlite_roots_with_a_scrubbed_environment() {
        let root = temp_root("managed-codex");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let executable = if cfg!(windows) {
            PathBuf::from(r"C:\Windows\System32\cmd.exe")
        } else {
            PathBuf::from("/bin/sh")
        };
        let profile =
            ManagedCodexProfile::prepare(&root, "account-a", executable, &project).unwrap();
        let process = profile.process_config();

        assert_eq!(profile.profile_id(), "account-a");
        assert!(profile.profile_root().starts_with(root.join("codex")));
        assert!(process.clears_environment());
        assert_eq!(
            process.environment_value("CODEX_HOME"),
            Some(profile.codex_home.as_os_str())
        );
        assert_eq!(
            process.environment_value("CODEX_SQLITE_HOME"),
            Some(profile.sqlite_home.as_os_str())
        );
        assert!(process.environment_value("OPENAI_API_KEY").is_none());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for path in [&profile.profile_root, &profile.codex_home, &profile.sqlite_home] {
                assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o700);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_relative_executables() {
        let root = temp_root("managed-codex-invalid");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        assert!(matches!(
            ManagedCodexProfile::prepare(&root, "../escape", "/bin/codex", &project),
            Err(ManagedCodexError::InvalidProfileId)
        ));
        assert!(matches!(
            ManagedCodexProfile::prepare(&root, "profile", "codex", &project),
            Err(ManagedCodexError::ExecutableNotAbsolute)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
