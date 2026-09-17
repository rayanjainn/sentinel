//! In-memory conversation state shared by the engine (which appends turns and creates plans) and
//! the plan executor (which updates plan state). Model history and the rendered transcript are
//! kept separately: history carries tool results and provider replay data, the transcript is what
//! the chat panel shows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use sentinel_core::model::TimestampMs;
use sentinel_core::{CoreResult, SentinelError};

use crate::backend::ChatMessage;
use crate::events::TranscriptItem;
use crate::plan::Plan;

pub fn now_ms() -> TimestampMs {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Default)]
pub struct Conversation {
    pub id: String,
    pub messages: Vec<ChatMessage>,
    pub transcript: Vec<TranscriptItem>,
    pub plans: Vec<Plan>,
    pub created_at_ms: TimestampMs,
}

#[derive(Debug, Default)]
pub struct ConversationStore {
    conversations: Mutex<HashMap<String, Conversation>>,
}

impl ConversationStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Conversation>> {
        self.conversations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn create(&self) -> String {
        let id = new_id();
        self.lock().insert(
            id.clone(),
            Conversation {
                id: id.clone(),
                created_at_ms: now_ms(),
                ..Default::default()
            },
        );
        id
    }

    pub fn exists(&self, id: &str) -> bool {
        self.lock().contains_key(id)
    }

    pub fn with<T>(&self, id: &str, f: impl FnOnce(&mut Conversation) -> T) -> CoreResult<T> {
        let mut conversations = self.lock();
        let conversation = conversations
            .get_mut(id)
            .ok_or_else(|| SentinelError::invalid(format!("unknown conversation `{id}`")))?;
        Ok(f(conversation))
    }

    pub fn messages(&self, id: &str) -> CoreResult<Vec<ChatMessage>> {
        self.with(id, |c| c.messages.clone())
    }

    pub fn transcript(&self, id: &str) -> CoreResult<Vec<TranscriptItem>> {
        self.with(id, |c| c.transcript.clone())
    }

    pub fn plan(&self, plan_id: &str) -> CoreResult<Plan> {
        self.lock()
            .values()
            .flat_map(|c| c.plans.iter())
            .find(|p| p.id == plan_id)
            .cloned()
            .ok_or_else(|| SentinelError::invalid(format!("unknown plan `{plan_id}`")))
    }

    pub fn add_plan(&self, plan: Plan) -> CoreResult<()> {
        self.with(&plan.conversation_id.clone(), |c| {
            c.transcript
                .push(TranscriptItem::Plan { plan: plan.clone() });
            c.plans.push(plan);
        })
    }

    /// Replaces a stored plan and its transcript entry.
    pub fn update_plan(&self, plan: &Plan) -> CoreResult<()> {
        self.with(&plan.conversation_id, |c| {
            if let Some(stored) = c.plans.iter_mut().find(|p| p.id == plan.id) {
                *stored = plan.clone();
            }
            for item in &mut c.transcript {
                if let TranscriptItem::Plan { plan: shown } = item
                    && shown.id == plan.id
                {
                    *shown = plan.clone();
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanStatus;

    #[test]
    fn plans_update_in_place_in_history_and_transcript() {
        let store = ConversationStore::new();
        let id = store.create();
        let mut plan = Plan {
            id: "p1".into(),
            conversation_id: id.clone(),
            request: "free space".into(),
            explanation: String::new(),
            actions: Vec::new(),
            status: PlanStatus::AwaitingReview,
            created_at_ms: now_ms(),
        };
        store.add_plan(plan.clone()).unwrap();
        plan.status = PlanStatus::Completed;
        store.update_plan(&plan).unwrap();
        assert_eq!(store.plan("p1").unwrap().status, PlanStatus::Completed);
        assert!(
            matches!(&store.transcript(&id).unwrap()[0], TranscriptItem::Plan { plan } if plan.status == PlanStatus::Completed)
        );
        assert!(store.plan("nope").is_err());
        assert!(store.transcript("nope").is_err());
    }
}
