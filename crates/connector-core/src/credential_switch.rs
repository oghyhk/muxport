use adapter_api::{
    AccountState, AdapterError, AgentAdapter, CredentialMaterial, CredentialValidation,
};
use credential_vault::{PersistentVault, VaultError};
use muxport_protocol::CredentialStatus;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialSwitchResult {
    pub profile_id: String,
    pub provider_id: String,
    pub account_fingerprint: Option<String>,
    pub activated_at_ms: i64,
}

#[derive(Debug, Error)]
pub enum CredentialSwitchError {
    #[error("credential profile does not exist: {0}")]
    ProfileNotFound(String),
    #[error("credential profile provider {profile_provider} is incompatible with runtime provider {runtime_provider}")]
    IncompatibleProvider {
        profile_provider: String,
        runtime_provider: String,
    },
    #[error("credential profile is not staged")]
    NotStaged,
    #[error("credential validation did not accept the staged secret")]
    ValidationRejected,
    #[error("credential runtime operation failed: {0}")]
    Adapter(#[from] AdapterError),
    #[error("credential vault operation failed: {0}")]
    Vault(#[from] VaultError),
    #[error("credential activation failed; the prior runtime credential was restored: {0}")]
    ActivationRolledBack(String),
    #[error("credential activation failed and runtime rollback also failed: activation={activation}; rollback={rollback}")]
    RollbackFailed {
        activation: String,
        rollback: String,
    },
}

#[async_trait::async_trait]
pub trait CredentialRuntime: Send + Sync {
    async fn validate(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError>;
    async fn activate(
        &self,
        profile_id: &str,
        credential: &CredentialMaterial,
    ) -> Result<(), AdapterError>;
    async fn read_account(&self, provider_id: &str) -> Result<AccountState, AdapterError>;
}

#[async_trait::async_trait]
impl<T> CredentialRuntime for T
where
    T: AgentAdapter + Send + Sync + ?Sized,
{
    async fn validate(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError> {
        self.validate_credential(credential).await
    }

    async fn activate(
        &self,
        profile_id: &str,
        credential: &CredentialMaterial,
    ) -> Result<(), AdapterError> {
        self.activate_credential(profile_id, credential).await
    }

    async fn read_account(&self, provider_id: &str) -> Result<AccountState, AdapterError> {
        self.read_account_state(provider_id).await
    }
}

/// Activates a staged API key without committing the vault assignment until
/// the managed runtime accepts it and provider readback confirms the result.
///
/// On every failure after runtime mutation begins, the prior encrypted secret
/// is reactivated. A rollback failure is reported explicitly and the vault
/// remains staged so a restart reconciler can retry from durable state.
pub async fn activate_staged_credential<R: CredentialRuntime + ?Sized>(
    runtime: &R,
    vault: &mut PersistentVault,
    profile_id: &str,
    runtime_provider_id: &str,
) -> Result<CredentialSwitchResult, CredentialSwitchError> {
    let summary = vault
        .get_profile(profile_id)
        .ok_or_else(|| CredentialSwitchError::ProfileNotFound(profile_id.to_owned()))?;
    if summary.provider != runtime_provider_id {
        return Err(CredentialSwitchError::IncompatibleProvider {
            profile_provider: summary.provider,
            runtime_provider: runtime_provider_id.to_owned(),
        });
    }
    if summary.status != CredentialStatus::Staged || !summary.has_staged_credential {
        return Err(CredentialSwitchError::NotStaged);
    }

    let prior_secret = vault.decrypt_active_secret(profile_id)?;
    let staged_secret = vault.decrypt_staged_secret(profile_id)?;
    let prior =
        CredentialMaterial::api_key(runtime_provider_id, prior_secret.expose_secret())?;
    let staged =
        CredentialMaterial::api_key(runtime_provider_id, staged_secret.expose_secret())?;

    let validation = runtime.validate(&staged).await?;
    if validation.provider_id != runtime_provider_id
        || !matches!(
            validation.status,
            CredentialStatus::Staged | CredentialStatus::Active
        )
    {
        return Err(CredentialSwitchError::ValidationRejected);
    }

    if let Err(activation) = runtime.activate(profile_id, &staged).await {
        return Err(rollback_after_failure(
            runtime,
            profile_id,
            &prior,
            activation.to_string(),
        )
        .await);
    }

    let account = match runtime.read_account(runtime_provider_id).await {
        Ok(account)
            if account.connected && account.provider_id == runtime_provider_id =>
        {
            account
        }
        Ok(_) => {
            return Err(rollback_after_failure(
                runtime,
                profile_id,
                &prior,
                "provider readback did not confirm the activated credential".into(),
            )
            .await);
        }
        Err(error) => {
            return Err(rollback_after_failure(
                runtime,
                profile_id,
                &prior,
                error.to_string(),
            )
            .await);
        }
    };

    let activated_at_ms = chrono::Utc::now().timestamp_millis();
    if let Err(error) = vault.mark_staged_validated(profile_id, activated_at_ms) {
        return Err(rollback_after_failure(
            runtime,
            profile_id,
            &prior,
            error.to_string(),
        )
        .await);
    }
    if let Err(error) = vault.activate_credential(profile_id) {
        return Err(rollback_after_failure(
            runtime,
            profile_id,
            &prior,
            error.to_string(),
        )
        .await);
    }

    Ok(CredentialSwitchResult {
        profile_id: profile_id.to_owned(),
        provider_id: runtime_provider_id.to_owned(),
        account_fingerprint: account
            .account_fingerprint
            .or(validation.account_fingerprint),
        activated_at_ms,
    })
}

async fn rollback_after_failure<R: CredentialRuntime + ?Sized>(
    runtime: &R,
    profile_id: &str,
    prior: &CredentialMaterial,
    activation: String,
) -> CredentialSwitchError {
    match runtime.activate(profile_id, prior).await {
        Ok(()) => CredentialSwitchError::ActivationRolledBack(activation),
        Err(rollback) => CredentialSwitchError::RollbackFailed {
            activation,
            rollback: rollback.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use credential_vault::{CredentialEnrollment, KeyEncryptionKey};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeCredentialRuntime {
        activations: Mutex<Vec<String>>,
        fail_candidate: AtomicBool,
        fail_rollback: AtomicBool,
        disconnected_readback: AtomicBool,
        activation_count: AtomicUsize,
    }

    impl FakeCredentialRuntime {
        fn healthy() -> Self {
            Self {
                activations: Mutex::new(Vec::new()),
                fail_candidate: AtomicBool::new(false),
                fail_rollback: AtomicBool::new(false),
                disconnected_readback: AtomicBool::new(false),
                activation_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl CredentialRuntime for FakeCredentialRuntime {
        async fn validate(
            &self,
            credential: &CredentialMaterial,
        ) -> Result<CredentialValidation, AdapterError> {
            Ok(CredentialValidation {
                status: CredentialStatus::Staged,
                provider_id: credential.provider_id().to_owned(),
                account_fingerprint: Some("account-test".into()),
            })
        }

        async fn activate(
            &self,
            _profile_id: &str,
            credential: &CredentialMaterial,
        ) -> Result<(), AdapterError> {
            let index = self.activation_count.fetch_add(1, Ordering::SeqCst);
            let secret = credential.secret_utf8()?.to_owned();
            self.activations.lock().unwrap().push(secret);
            if index == 0 && self.fail_candidate.load(Ordering::SeqCst) {
                return Err(AdapterError::CredentialInvalid("candidate rejected".into()));
            }
            if index > 0 && self.fail_rollback.load(Ordering::SeqCst) {
                return Err(AdapterError::OutcomeUnknown("rollback unavailable".into()));
            }
            Ok(())
        }

        async fn read_account(
            &self,
            provider_id: &str,
        ) -> Result<AccountState, AdapterError> {
            Ok(AccountState {
                provider_id: provider_id.to_owned(),
                connected: !self.disconnected_readback.load(Ordering::SeqCst),
                account_fingerprint: Some("account-test".into()),
            })
        }
    }

    fn test_vault(label: &str) -> (PersistentVault, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "muxport-switch-{label}-{}-{}.sealed",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let salt = [7_u8; 16];
        let kek = KeyEncryptionKey::derive_from_passphrase(b"test-passphrase", &salt).unwrap();
        let mut vault = PersistentVault::open_or_create(&path, "host-test", kek).unwrap();
        vault
            .enroll_credential(
                CredentialEnrollment {
                    profile_id: "profile-a".into(),
                    display_name: "Go A".into(),
                    provider: "opencode".into(),
                    credential_type: "api_key".into(),
                    account_fingerprint: "account-old".into(),
                    created_at_ms: 1,
                    last_validated_at_ms: 1,
                },
                b"old-key",
            )
            .unwrap();
        vault.stage_credential("profile-a", b"new-key").unwrap();
        (vault, path)
    }

    #[tokio::test]
    async fn commits_vault_only_after_runtime_activation_and_readback() {
        let (mut vault, path) = test_vault("success");
        let runtime = FakeCredentialRuntime::healthy();
        let result =
            activate_staged_credential(&runtime, &mut vault, "profile-a", "opencode")
                .await
                .unwrap();
        assert_eq!(result.account_fingerprint.as_deref(), Some("account-test"));
        assert_eq!(
            vault
                .decrypt_active_secret("profile-a")
                .unwrap()
                .expose_secret(),
            b"new-key"
        );
        assert!(vault.get_profile("profile-a").unwrap().has_rollback_credential);
        assert_eq!(
            runtime.activations.lock().unwrap().as_slice(),
            ["new-key"]
        );
        drop(vault);
        fs_cleanup(&path);
    }

    #[tokio::test]
    async fn failed_candidate_reactivates_old_secret_and_keeps_vault_staged() {
        let (mut vault, path) = test_vault("rollback");
        let runtime = FakeCredentialRuntime::healthy();
        runtime.fail_candidate.store(true, Ordering::SeqCst);
        assert!(matches!(
            activate_staged_credential(&runtime, &mut vault, "profile-a", "opencode").await,
            Err(CredentialSwitchError::ActivationRolledBack(_))
        ));
        assert_eq!(
            runtime.activations.lock().unwrap().as_slice(),
            ["new-key", "old-key"]
        );
        let summary = vault.get_profile("profile-a").unwrap();
        assert_eq!(summary.status, CredentialStatus::Staged);
        assert!(summary.has_staged_credential);
        assert_eq!(
            vault
                .decrypt_active_secret("profile-a")
                .unwrap()
                .expose_secret(),
            b"old-key"
        );
        drop(vault);
        fs_cleanup(&path);
    }

    #[tokio::test]
    async fn rollback_failure_is_explicit_and_never_commits_vault() {
        let (mut vault, path) = test_vault("rollback-failed");
        let runtime = FakeCredentialRuntime::healthy();
        runtime.fail_candidate.store(true, Ordering::SeqCst);
        runtime.fail_rollback.store(true, Ordering::SeqCst);
        assert!(matches!(
            activate_staged_credential(&runtime, &mut vault, "profile-a", "opencode").await,
            Err(CredentialSwitchError::RollbackFailed { .. })
        ));
        assert_eq!(
            vault.get_profile("profile-a").unwrap().status,
            CredentialStatus::Staged
        );
        drop(vault);
        fs_cleanup(&path);
    }

    fn fs_cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("sealed-wal"));
        let _ = std::fs::remove_file(path.with_extension("sealed-shm"));
    }
}
