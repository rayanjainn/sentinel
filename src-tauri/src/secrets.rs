//! API keys in the OS credential store (macOS Keychain, Windows Credential Manager, Secret
//! Service on Linux) via `keyring`. One entry per provider under `settings::KEYCHAIN_SERVICE`.
//! Keys are never logged, returned to the frontend, or written anywhere else.

use sentinel_agent::settings::{KEYCHAIN_SERVICE, ProviderId};
use sentinel_core::{CoreResult, SentinelError};

pub trait SecretStore: Send + Sync {
    fn get(&self, provider: ProviderId) -> CoreResult<Option<String>>;
    fn set(&self, provider: ProviderId, secret: &str) -> CoreResult<()>;
    /// Deleting a key that does not exist succeeds.
    fn delete(&self, provider: ProviderId) -> CoreResult<()>;
}

pub struct Keychain {
    service: String,
}

impl Keychain {
    pub fn app() -> Self {
        Self::with_service(KEYCHAIN_SERVICE)
    }

    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, provider: ProviderId) -> CoreResult<keyring::Entry> {
        keyring::Entry::new(&self.service, provider.keychain_account()).map_err(map_error)
    }
}

/// Maps store failures to specific messages. Error text never includes secret material.
fn map_error(err: keyring::Error) -> SentinelError {
    match err {
        keyring::Error::NoStorageAccess(_) => SentinelError::PermissionDenied {
            operation: "access the system keychain".into(),
            target: None,
            hint: Some(
                "Unlock the keychain and allow Sentinel to use it when asked, then try again."
                    .into(),
            ),
        },
        keyring::Error::NoDefaultStore => SentinelError::Unavailable {
            feature: "secure key storage".into(),
            reason: "no system credential store is available (on Linux, install and unlock a Secret Service provider such as GNOME Keyring or KWallet)".into(),
        },
        keyring::Error::PlatformFailure(inner) => SentinelError::Internal {
            detail: format!("the system keychain reported an error: {inner}"),
        },
        keyring::Error::BadEncoding(_) | keyring::Error::BadDataFormat(..) => {
            SentinelError::Internal {
                detail: "the stored key is unreadable; remove it and add it again".into(),
            }
        }
        other => SentinelError::Internal {
            detail: format!("keychain error: {}", describe(&other)),
        },
    }
}

fn describe(err: &keyring::Error) -> &'static str {
    match err {
        keyring::Error::TooLong(..) => "value too long",
        keyring::Error::Invalid(..) => "invalid entry",
        keyring::Error::Ambiguous(_) => "more than one matching entry",
        keyring::Error::NotSupportedByStore(_) => "operation not supported",
        _ => "unexpected failure",
    }
}

impl SecretStore for Keychain {
    fn get(&self, provider: ProviderId) -> CoreResult<Option<String>> {
        match self.entry(provider)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(map_error(err)),
        }
    }

    fn set(&self, provider: ProviderId, secret: &str) -> CoreResult<()> {
        self.entry(provider)?
            .set_password(secret)
            .map_err(map_error)
    }

    fn delete(&self, provider: ProviderId) -> CoreResult<()> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(map_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real OS credential store under a unique, test-only service name and removes
    /// the entry afterwards. Ignored by default (CI runners have no unlocked keychain).
    #[test]
    #[ignore = "uses the real OS keychain; run with --ignored on a desktop session"]
    fn keychain_round_trip_under_test_service() {
        let service = format!("{KEYCHAIN_SERVICE}.test-{}", std::process::id());
        let store = Keychain::with_service(service);
        store.delete(ProviderId::Gemini).unwrap();
        assert_eq!(store.get(ProviderId::Gemini).unwrap(), None);
        store.set(ProviderId::Gemini, "test-secret-value").unwrap();
        assert_eq!(
            store.get(ProviderId::Gemini).unwrap().as_deref(),
            Some("test-secret-value")
        );
        assert_eq!(store.get(ProviderId::Anthropic).unwrap(), None);
        store.delete(ProviderId::Gemini).unwrap();
        assert_eq!(store.get(ProviderId::Gemini).unwrap(), None);
    }
}
