//! Single-use, time-limited action tokens.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::action::ActionPreview;
use crate::error::{CoreResult, SentinelError};

pub const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);

pub struct TokenStore {
    ttl: Duration,
    pending: Mutex<HashMap<String, (Instant, ActionPreview)>>,
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::with_ttl(TOKEN_TTL)
    }
}

impl TokenStore {
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_token() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    pub fn insert(&self, preview: ActionPreview) {
        let mut pending = self.pending.lock();
        let ttl = self.ttl;
        pending.retain(|_, (created, _)| created.elapsed() < ttl);
        pending.insert(preview.token.clone(), (Instant::now(), preview));
    }

    /// Removes the token whether or not it is still valid, so it can never be used twice.
    pub fn take(&self, token: &str) -> CoreResult<ActionPreview> {
        let entry = self.pending.lock().remove(token);
        match entry {
            Some((created, preview)) if created.elapsed() < self.ttl => Ok(preview),
            _ => Err(SentinelError::ActionTokenInvalid),
        }
    }

    pub fn len(&self) -> usize {
        self.pending.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, ActionRisk, Origin, Reversibility};

    fn preview(token: &str) -> ActionPreview {
        ActionPreview {
            token: token.to_owned(),
            action: Action::TrashPaths { paths: vec![] },
            origin: Origin::User,
            title: "t".into(),
            description: "d".into(),
            targets: vec![],
            impact: vec![],
            estimated_bytes_freed: None,
            risk: ActionRisk::Moderate,
            reversibility: Reversibility::Irreversible,
            warnings: vec![],
            requires_elevation: false,
            created_at_ms: 0,
            expires_at_ms: 0,
        }
    }

    #[test]
    fn tokens_are_single_use() {
        let store = TokenStore::default();
        store.insert(preview("a"));
        assert!(store.take("a").is_ok());
        assert_eq!(store.take("a"), Err(SentinelError::ActionTokenInvalid));
        assert_eq!(
            store.take("unknown"),
            Err(SentinelError::ActionTokenInvalid)
        );
    }

    #[test]
    fn tokens_expire() {
        let store = TokenStore::with_ttl(Duration::from_millis(20));
        store.insert(preview("b"));
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(store.take("b"), Err(SentinelError::ActionTokenInvalid));
        assert!(store.is_empty());
    }

    #[test]
    fn generated_tokens_are_unique() {
        assert_ne!(TokenStore::new_token(), TokenStore::new_token());
    }
}
