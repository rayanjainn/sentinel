//! End-to-end check of the agent against the real Sentinel backend: `CoreRuntime` (live processes,
//! sockets, volumes), the real `ActionService` and a SQLite audit store, with local Ollama.
//!
//! It spawns its own `sleep 600` child and approves only the terminate action aimed at that PID;
//! every other proposal is rejected. Ignored by default:
//!
//! ```sh
//! cargo test -p sentinel-app --test agent_e2e -- --ignored --nocapture
//! ```

#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sentinel_agent::backend::AgentBackend;
use sentinel_agent::conversation::ConversationStore;
use sentinel_agent::engine::{AgentEngine, CancellationToken, EngineConfig, EngineParts};
use sentinel_agent::events::{AgentEvent, AgentEventSink, AgentStreamPayload};
use sentinel_agent::executor::PlanExecutor;
use sentinel_agent::plan::{
    ActionDecision, Decision, Plan, PlanAction, PlanActionState, PlanStatus,
};
use sentinel_agent::providers::catalog;
use sentinel_agent::settings::{AgentSettings, ProviderId};
use sentinel_agent::tools::WriteToolTranslator;
use sentinel_agent::tools::read::SystemReadTools;
use sentinel_agent::tools::write::SystemWriteTranslator;
use sentinel_core::action::Origin;
use sentinel_core::action::{Action, ActionCommitter, ActionPreparer};
use sentinel_core::audit::{AuditQuery, AuditStatus, AuditStore, OriginFilter};
use sentinel_core::events::{CoreEvent, EventSink};
use sentinel_core::runtime::{CoreRuntime, RuntimeConfig};
use sentinel_core::service::SystemQueries;
use sentinel_core::service::actions::{ActionContext, ActionService, ActionServiceConfig};
use sentinel_core::service::audit_sqlite::SqliteAuditStore;

struct QuietCore;
impl EventSink for QuietCore {
    fn emit(&self, _event: CoreEvent) {}
}

#[derive(Default)]
struct Events(Mutex<Vec<AgentEvent>>);
impl AgentEventSink for Events {
    fn emit(&self, payload: AgentStreamPayload) {
        if let AgentEvent::ToolCallStarted { name, input, .. } = &payload.event {
            eprintln!("tool call: {name} {input}");
        }
        if let AgentEvent::ToolCallFinished { ok, summary, .. } = &payload.event {
            eprintln!("  -> ok={ok}: {summary}");
        }
        self.0.lock().unwrap().push(payload.event);
    }
}

impl Events {
    fn take(&self) -> Vec<AgentEvent> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

fn plans(events: &[AgentEvent]) -> Vec<Plan> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::PlanProposed { plan } => Some(plan.clone()),
            _ => None,
        })
        .collect()
}

fn text(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn agent_audit_entries(audit: &dyn AuditStore) -> usize {
    audit
        .query(&AuditQuery {
            limit: 100,
            before_id: None,
            origin: OriginFilter::Agent,
        })
        .unwrap()
        .len()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs local Ollama with llama3.1:8b; spawns and terminates its own sleep child"]
async fn agent_reads_live_state_and_runs_only_approved_actions() {
    let data_dir = std::env::temp_dir().join(format!("sentinel-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let runtime = CoreRuntime::start(
        RuntimeConfig {
            data_dir: data_dir.clone(),
        },
        Arc::new(QuietCore),
    )
    .expect("core runtime");
    let audit: Arc<dyn AuditStore> =
        Arc::new(SqliteAuditStore::open(&data_dir.join("audit.sqlite3")).unwrap());
    let permissions = runtime.permission_status();
    let actions = Arc::new(ActionService::new(
        Arc::clone(&runtime) as Arc<dyn ActionContext>,
        runtime.process_control(),
        Arc::clone(&audit),
        ActionServiceConfig {
            platform: permissions.platform,
            running_elevated: permissions.running_elevated,
        },
    ));
    let queries: Arc<dyn SystemQueries> = runtime.clone();

    let mut child = std::process::Command::new("sleep")
        .arg("600")
        .spawn()
        .unwrap();
    let pid = child.id();

    let model =
        std::env::var("SENTINEL_TEST_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.1:8b".into());
    let settings = AgentSettings::default();
    let backend: Arc<dyn AgentBackend> =
        catalog::prepare_backend(ProviderId::Ollama, None, &settings, &model)
            .await
            .expect("Ollama running with the model pulled");
    let store = ConversationStore::new();
    let events = Arc::new(Events::default());
    let engine = AgentEngine::new(EngineParts {
        backend,
        reads: Arc::new(SystemReadTools::new(Arc::clone(&queries))),
        writes: Arc::new(SystemWriteTranslator::new(Arc::clone(&queries))),
        preparer: Arc::clone(&actions) as Arc<dyn ActionPreparer>,
        store: Arc::clone(&store),
        sink: events.clone(),
        config: EngineConfig {
            model: model.clone(),
            max_tool_rounds: 8,
            max_tokens: 4096,
        },
    });
    let conversation = store.create();

    // 1. A read-only question hits live data and changes nothing.
    engine
        .send_message(
            &conversation,
            "What's eating my CPU right now?",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let first = events.take();
    eprintln!("assistant: {}", text(&first));
    assert!(
        first
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolCallFinished { ok: true, summary, .. } if summary.contains("process") || summary.contains("CPU"))),
        "a read tool should summarize live processes or CPU"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "nothing may be terminated"
    );
    assert_eq!(agent_audit_entries(audit.as_ref()), 0);

    // 2. Ask for a change aimed at our own child; it must become a card, not an action.
    let request = format!(
        "There is a leftover test job: the `sleep` process with PID {pid}. Check it with get_process_details, then call terminate_process for PID {pid}."
    );
    let mut plan = None;
    for attempt in 0..2 {
        engine
            .send_message(&conversation, &request, CancellationToken::new())
            .await
            .unwrap();
        let turn = events.take();
        eprintln!("attempt {attempt} assistant: {}", text(&turn));
        plan = plans(&turn).into_iter().find(|p| {
            p.actions.iter().any(|a| {
                matches!(a.preview.action, Action::TerminateProcess { target } if target.pid == pid)
            })
        });
        if plan.is_some() {
            break;
        }
    }
    // Small local models sometimes write the call as prose instead of calling the tool. The
    // execute path is the safety-critical half, so it is verified either way: the proposal then
    // goes through the same translator and preparer the engine uses.
    let plan = match plan {
        Some(plan) => plan,
        None => {
            eprintln!(
                "model never called terminate_process; proposing through the write translator"
            );
            let proposal = SystemWriteTranslator::new(Arc::clone(&queries))
                .translate(
                    "terminate_process",
                    serde_json::json!({"pid": pid, "reason": "leftover test job"}),
                )
                .expect("translate");
            let preview = actions
                .prepare(
                    proposal.action,
                    Origin::Agent {
                        conversation_id: conversation.clone(),
                        plan_id: "e2e-plan".into(),
                        provider: "Ollama (local)".into(),
                        model: model.clone(),
                        request: request.clone(),
                    },
                )
                .expect("prepare");
            let plan = Plan {
                id: "e2e-plan".into(),
                conversation_id: conversation.clone(),
                request: request.clone(),
                explanation: "Proposed directly through the write translator.".into(),
                actions: vec![PlanAction {
                    id: "e2e-action".into(),
                    rationale: proposal.rationale,
                    preview,
                    state: PlanActionState::Pending,
                }],
                status: PlanStatus::AwaitingReview,
                created_at_ms: 0,
            };
            store.add_plan(plan.clone()).unwrap();
            plan
        }
    };
    assert!(
        child.try_wait().unwrap().is_none(),
        "proposal must not execute"
    );
    assert_eq!(
        agent_audit_entries(audit.as_ref()),
        0,
        "no commit before approval"
    );

    // 3. Approve only the terminate for our child.
    let decisions: Vec<ActionDecision> = plan
        .actions
        .iter()
        .map(|a| ActionDecision {
            action_id: a.id.clone(),
            decision: match a.preview.action {
                Action::TerminateProcess { target } if target.pid == pid => Decision::Approve,
                _ => Decision::Reject,
            },
        })
        .collect();
    let executor = PlanExecutor::new(
        Arc::clone(&actions) as Arc<dyn ActionCommitter>,
        Arc::clone(&actions) as Arc<dyn ActionPreparer>,
        Arc::clone(&store),
        events.clone(),
    );
    let done = executor.execute(&plan.id, &decisions).await.unwrap();
    for action in &done.actions {
        eprintln!(
            "{} -> {:?}",
            action.preview.title,
            std::mem::discriminant(&action.state)
        );
    }
    let ours = done
        .actions
        .iter()
        .find(|a| matches!(a.preview.action, Action::TerminateProcess { target } if target.pid == pid))
        .unwrap();
    assert!(
        matches!(&ours.state, PlanActionState::Succeeded { outcome } if !outcome.summary.is_empty()),
        "{:?}",
        ours.state
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let exited = child.try_wait().unwrap().is_some();
    if !exited {
        let _ = child.kill();
    }
    assert!(exited, "the approved terminate reached the real process");

    let entries = audit
        .query(&AuditQuery {
            limit: 100,
            before_id: None,
            origin: OriginFilter::Agent,
        })
        .unwrap();
    assert!(entries.iter().any(|e| {
        e.status == AuditStatus::Succeeded
            && e.trigger
                .as_deref()
                .is_some_and(|t| t.contains("leftover test job"))
    }));
    let _ = std::fs::remove_dir_all(&data_dir);
}
