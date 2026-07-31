use crate::OpenCodeAdapter;
use adapter_api::AdapterError;
#[cfg(test)]
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use thiserror::Error;
use tokio::process::{Child, Command};

#[derive(Debug, Error)]
pub enum ManagedOpenCodeError {
    #[error("invalid managed OpenCode profile id")]
    InvalidProfileId,
    #[error("managed OpenCode executable path must be absolute")]
    ExecutableNotAbsolute,
    #[error("managed OpenCode project directory must be absolute")]
    ProjectDirectoryNotAbsolute,
    #[error("managed OpenCode server password must not be empty")]
    EmptyServerPassword,
    #[error("managed OpenCode provider id is invalid")]
    InvalidProviderId,
    #[error("managed OpenCode profile path contains a symbolic link: {0}")]
    SymbolicLink(PathBuf),
    #[error("managed OpenCode profile path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("managed OpenCode profile I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("managed OpenCode adapter configuration failed: {0}")]
    Adapter(#[from] AdapterError),
}

/// A connector-owned OpenCode profile with a fully separate home, data,
/// configuration, cache, and state root.
///
/// No vendor auth file is copied or edited by Muxport. OpenCode itself writes
/// its isolated auth state through its supported HTTP auth endpoint.
#[derive(Clone, Debug)]
pub struct ManagedOpenCodeProfile {
    profile_id: String,
    executable: PathBuf,
    project_directory: PathBuf,
    profile_root: PathBuf,
    home_root: PathBuf,
    data_root: PathBuf,
    config_root: PathBuf,
    cache_root: PathBuf,
    state_root: PathBuf,
    port: u16,
}

impl ManagedOpenCodeProfile {
    pub fn prepare(
        profiles_root: impl AsRef<Path>,
        profile_id: impl Into<String>,
        executable: impl Into<PathBuf>,
        project_directory: impl Into<PathBuf>,
        port: u16,
    ) -> Result<Self, ManagedOpenCodeError> {
        let profile_id = profile_id.into();
        if !valid_profile_id(&profile_id) {
            return Err(ManagedOpenCodeError::InvalidProfileId);
        }
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(ManagedOpenCodeError::ExecutableNotAbsolute);
        }
        let project_directory = project_directory.into();
        if !project_directory.is_absolute() {
            return Err(ManagedOpenCodeError::ProjectDirectoryNotAbsolute);
        }

        let vendor_root = profiles_root.as_ref().join("opencode");
        let profile_root = vendor_root.join(&profile_id);
        let home_root = profile_root.join("home");
        let data_root = profile_root.join("data");
        let config_root = profile_root.join("config");
        let cache_root = profile_root.join("cache");
        let state_root = profile_root.join("state");
        for directory in [
            &vendor_root,
            &profile_root,
            &home_root,
            &data_root,
            &config_root,
            &cache_root,
            &state_root,
            &config_root.join("opencode"),
        ] {
            create_private_directory(directory)?;
        }

        Ok(Self {
            profile_id,
            executable,
            project_directory,
            profile_root,
            home_root,
            data_root,
            config_root,
            cache_root,
            state_root,
            port,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn adapter(
        &self,
        server_password: impl Into<String>,
    ) -> Result<OpenCodeAdapter, ManagedOpenCodeError> {
        let server_password = server_password.into();
        if server_password.is_empty() {
            return Err(ManagedOpenCodeError::EmptyServerPassword);
        }
        Ok(OpenCodeAdapter::new_managed(
            self.base_url(),
            Some(server_password),
            self.profile_id.clone(),
        )?)
    }

    /// Starts OpenCode bound to loopback with an isolated environment.
    ///
    /// The child does not inherit arbitrary provider environment variables
    /// from the connector. Only a small OS bootstrap allowlist is copied.
    pub fn spawn(&self, server_password: &str) -> Result<Child, ManagedOpenCodeError> {
        if server_password.is_empty() {
            return Err(ManagedOpenCodeError::EmptyServerPassword);
        }
        let mut command = Command::new(&self.executable);
        self.configure_isolated_environment(&mut command);
        command
            .arg("serve")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(self.port.to_string())
            .current_dir(&self.project_directory)
            .env("OPENCODE_SERVER_USERNAME", "opencode")
            .env("OPENCODE_SERVER_PASSWORD", server_password)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn().map_err(ManagedOpenCodeError::Io)
    }

    /// Runs OpenCode's supported interactive authentication command inside
    /// this isolated profile. Muxport never reads or writes the vendor auth
    /// file and never receives the entered secret.
    pub async fn auth_login(
        &self,
        provider_id: &str,
    ) -> Result<std::process::ExitStatus, ManagedOpenCodeError> {
        if provider_id.is_empty()
            || provider_id.len() > 256
            || provider_id.chars().any(char::is_control)
        {
            return Err(ManagedOpenCodeError::InvalidProviderId);
        }
        let mut command = Command::new(&self.executable);
        command
            .arg("auth")
            .arg("login")
            .arg("--provider")
            .arg(provider_id)
            .current_dir(&self.project_directory)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        self.configure_isolated_environment(&mut command);
        command
            .status()
            .await
            .map_err(ManagedOpenCodeError::Io)
    }

    fn configure_isolated_environment(&self, command: &mut Command) {
        command
            .env_clear()
            .env("HOME", &self.home_root)
            .env("USERPROFILE", &self.home_root)
            .env("XDG_DATA_HOME", &self.data_root)
            .env("XDG_CONFIG_HOME", &self.config_root)
            .env("XDG_CACHE_HOME", &self.cache_root)
            .env("XDG_STATE_HOME", &self.state_root)
            .env("OPENCODE_CONFIG_DIR", self.config_root.join("opencode"));
        for name in bootstrap_environment_names() {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
    }

    #[cfg(test)]
    fn isolated_environment(&self) -> Vec<(OsString, OsString)> {
        vec![
            (OsString::from("HOME"), self.home_root.clone().into_os_string()),
            (
                OsString::from("USERPROFILE"),
                self.home_root.clone().into_os_string(),
            ),
            (
                OsString::from("XDG_DATA_HOME"),
                self.data_root.clone().into_os_string(),
            ),
            (
                OsString::from("XDG_CONFIG_HOME"),
                self.config_root.clone().into_os_string(),
            ),
            (
                OsString::from("XDG_CACHE_HOME"),
                self.cache_root.clone().into_os_string(),
            ),
            (
                OsString::from("XDG_STATE_HOME"),
                self.state_root.clone().into_os_string(),
            ),
            (
                OsString::from("OPENCODE_CONFIG_DIR"),
                self.config_root.join("opencode").into_os_string(),
            ),
        ]
    }
}

fn valid_profile_id(profile_id: &str) -> bool {
    !profile_id.is_empty()
        && profile_id.len() <= 128
        && profile_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn create_private_directory(path: &Path) -> Result<(), ManagedOpenCodeError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(ManagedOpenCodeError::SymbolicLink(path.to_owned()));
        }
        if !metadata.is_dir() {
            return Err(ManagedOpenCodeError::NotDirectory(path.to_owned()));
        }
    } else {
        fs::create_dir_all(path)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(ManagedOpenCodeError::SymbolicLink(path.to_owned()));
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
    fn prepares_separate_profile_roots_without_inheriting_provider_environment() {
        let root = temp_root("managed-opencode");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let executable = if cfg!(windows) {
            PathBuf::from(r"C:\Windows\System32\cmd.exe")
        } else {
            PathBuf::from("/bin/sh")
        };
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "go-account-a", executable, &project, 43111)
                .unwrap();

        assert_eq!(profile.profile_id(), "go-account-a");
        assert_eq!(profile.base_url(), "http://127.0.0.1:43111");
        assert!(profile.profile_root().starts_with(root.join("opencode")));
        let environment = profile.isolated_environment();
        assert!(environment
            .iter()
            .any(|(name, _)| name == "XDG_DATA_HOME"));
        assert!(!environment
            .iter()
            .any(|(name, _)| name == "OPENAI_API_KEY"));
        assert!(profile.adapter("test-server-password").is_ok());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for (_, path) in environment {
                assert_eq!(
                    fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_relative_paths_and_empty_passwords() {
        let root = temp_root("managed-opencode-invalid");
        let absolute_project = root.join("project");
        fs::create_dir_all(&absolute_project).unwrap();
        let executable = if cfg!(windows) {
            PathBuf::from(r"C:\Windows\System32\cmd.exe")
        } else {
            PathBuf::from("/bin/sh")
        };
        assert!(matches!(
            ManagedOpenCodeProfile::prepare(
                &root,
                "../escape",
                &executable,
                &absolute_project,
                43112
            ),
            Err(ManagedOpenCodeError::InvalidProfileId)
        ));
        assert!(matches!(
            ManagedOpenCodeProfile::prepare(
                &root,
                "profile",
                "opencode",
                &absolute_project,
                43112
            ),
            Err(ManagedOpenCodeError::ExecutableNotAbsolute)
        ));
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile", executable, absolute_project, 43112)
                .unwrap();
        assert!(matches!(
            profile.adapter(""),
            Err(ManagedOpenCodeError::EmptyServerPassword)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawned_runtime_is_loopback_authenticated_and_environment_isolated() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("managed-opencode-spawn");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let executable = root.join("fake-opencode");
        fs::write(
            &executable,
            concat!(
                "#!/bin/sh\n",
                "{\n",
                "  printf 'args=%s\\n' \"$*\"\n",
                "  printf 'home=%s\\n' \"$HOME\"\n",
                "  printf 'data=%s\\n' \"$XDG_DATA_HOME\"\n",
                "  printf 'config=%s\\n' \"$OPENCODE_CONFIG_DIR\"\n",
                "  printf 'password=%s\\n' \"${OPENCODE_SERVER_PASSWORD:+set}\"\n",
                "  printf 'inherited=%s\\n' \"${ANTHROPIC_API_KEY-unset}\"\n",
                "} > managed-launch.txt\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile-a", &executable, &project, 43113)
                .unwrap();

        let mut child = profile.spawn("test-server-password").unwrap();
        assert!(child.wait().await.unwrap().success());
        let evidence = fs::read_to_string(project.join("managed-launch.txt")).unwrap();
        assert!(evidence.contains("args=serve --hostname 127.0.0.1 --port 43113"));
        assert!(evidence.contains("password=set"));
        assert!(evidence.contains("inherited=unset"));
        assert!(evidence.contains(&format!("home={}", profile.home_root.display())));
        assert!(evidence.contains(&format!("data={}", profile.data_root.display())));
        assert!(evidence.contains(&format!(
            "config={}",
            profile.config_root.join("opencode").display()
        )));
        assert!(!evidence.contains("test-server-password"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supported_auth_cli_runs_inside_the_selected_profile() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("managed-opencode-auth");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let executable = root.join("fake-opencode");
        fs::write(
            &executable,
            concat!(
                "#!/bin/sh\n",
                "{\n",
                "  printf 'args=%s\\n' \"$*\"\n",
                "  printf 'data=%s\\n' \"$XDG_DATA_HOME\"\n",
                "  printf 'server_password=%s\\n' \"${OPENCODE_SERVER_PASSWORD-unset}\"\n",
                "} > managed-auth.txt\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile-a", &executable, &project, 43114)
                .unwrap();

        assert!(profile.auth_login("opencode").await.unwrap().success());
        let evidence = fs::read_to_string(project.join("managed-auth.txt")).unwrap();
        assert!(evidence.contains("args=auth login --provider opencode"));
        assert!(evidence.contains(&format!("data={}", profile.data_root.display())));
        assert!(evidence.contains("server_password=unset"));
        assert!(matches!(
            profile.auth_login("\n").await,
            Err(ManagedOpenCodeError::InvalidProviderId)
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MUXPORT_TEST_OPENCODE_PATH pointing to a real OpenCode executable"]
    async fn live_opencode_profile_restarts_with_the_same_isolated_state() {
        use adapter_api::{AgentAdapter, CredentialMaterial};

        let executable = std::env::var_os("MUXPORT_TEST_OPENCODE_PATH")
            .map(PathBuf::from)
            .expect("MUXPORT_TEST_OPENCODE_PATH");
        let root = temp_root("live-managed-opencode");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile-a", executable, &project, port)
                .unwrap();
        let password = "live-fixture-server-password";

        let discovery_adapter = profile.adapter(password).unwrap();
        for cycle in 0..2 {
            let mut child = profile.spawn(password).unwrap();
            let adapter = profile.adapter(password).unwrap();
            let mut healthy = false;
            for _ in 0..100 {
                if adapter.probe().await.is_ok() {
                    healthy = true;
                    break;
                }
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            assert!(healthy, "managed OpenCode did not become healthy");
            assert!(child.try_wait().unwrap().is_none());
            if cycle == 0 {
                let provider_id = discovery_adapter
                    .auth_methods()
                    .await
                    .unwrap()
                    .into_iter()
                    .find_map(|(provider_id, methods)| {
                        methods
                            .iter()
                            .any(|method| method.kind == "api")
                            .then_some(provider_id)
                    })
                    .expect("installed OpenCode exposed no API-key auth method");
                fs::write(root.join("discovered-provider-id"), &provider_id).unwrap();
                let credential = CredentialMaterial::api_key(
                    provider_id,
                    b"muxport-live-fixture-not-a-real-key",
                )
                .unwrap();
                adapter
                    .activate_credential("profile-a", &credential)
                    .await
                    .unwrap();
            } else {
                let provider_id =
                    fs::read_to_string(root.join("discovered-provider-id")).unwrap();
                assert!(
                    adapter
                        .read_account_state(&provider_id)
                        .await
                        .unwrap()
                        .connected,
                    "isolated provider state did not survive source restart"
                );
            }
            child.kill().await.unwrap();
            let _ = child.wait().await;
        }

        assert!(profile.data_root.join("opencode").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
