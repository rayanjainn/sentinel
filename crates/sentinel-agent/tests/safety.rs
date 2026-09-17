//! Safety invariants for the agent layer (docs/CONTRACT.md "Agent tools"), exercised with a
//! scripted model, fake system state and a fake action service that — like the real
//! `ActionService` — implements both `ActionPreparer` and `ActionCommitter` on one object.
//!
//! How "the engine cannot hold a committer" is enforced:
//! - `EngineParts` has no committer field and takes `Arc<dyn ActionPreparer>`; `dyn ActionPreparer`
//!   has no `Any` supertrait, so even an object that also implements `ActionCommitter` cannot be
//!   downcast back to one. Compile-fail doctests in `src/lib.rs` prove both constructions are
//!   rejected by the compiler.
//! - `engine_source_never_mentions_the_committer` below fails if `src/engine.rs` ever names
//!   `ActionCommitter` or calls `commit`/`reject`.
//! - Every scenario here passes the dual-trait fake as the preparer and asserts zero commits.

mod support;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use sentinel_agent::backend::{AgentBackend, StopReason};
use sentinel_agent::conversation::ConversationStore;
use sentinel_agent::engine::{AgentEngine, EngineConfig, EngineParts};
use sentinel_agent::events::AgentEvent;
use sentinel_agent::executor::PlanExecutor;
use sentinel_agent::plan::{ActionDecision, Decision, Plan, PlanActionState, PlanStatus};
use sentinel_agent::settings::ProviderId;
use sentinel_agent::tools::read::SystemReadTools;
use sentinel_agent::tools::write::SystemWriteTranslator;
use sentinel_core::SentinelError;
use sentinel_core::action::{Action, ActionCommitter, ActionPreparer};
use sentinel_core::model::{FileEntry, FileKindGroup};
use support::*;

const HOG_PID: u32 = 4821;

struct Harness {
    queries: Arc<FakeQueries>,
    actions: Arc<FakeActions>,
    backend: Arc<ScriptedBackend>,
    sink: Arc<RecordingSink>,
    store: Arc<ConversationStore>,
    engine: AgentEngine,
    executor: PlanExecutor,
    conversation: String,
}

impl Harness {
    fn new(backend: Arc<ScriptedBackend>) -> Self {
        let queries = FakeQueries::with_processes(vec![
            process(HOG_PID, "hog", 97.0, 1_000),
            process(812, "postgres", 1.5, 2_000),
        ]);
        let actions = FakeActions::new();
        let sink = RecordingSink::new();
        let store = ConversationStore::new();
        let engine = AgentEngine::new(EngineParts {
            backend: Arc::clone(&backend) as Arc<dyn AgentBackend>,
            reads: Arc::new(SystemReadTools::new(queries.clone())),
            writes: Arc::new(SystemWriteTranslator::new(queries.clone())),
            // The same object also implements ActionCommitter; the engine only ever sees this view.
            preparer: Arc::clone(&actions) as Arc<dyn ActionPreparer>,
            store: Arc::clone(&store),
            sink: sink.clone(),
            config: EngineConfig {
                model: "test-model".into(),
                max_tool_rounds: 8,
                max_tokens: 4096,
            },
        });
        let executor = PlanExecutor::new(
            Arc::clone(&actions) as Arc<dyn ActionCommitter>,
            Arc::clone(&actions) as Arc<dyn ActionPreparer>,
            Arc::clone(&store),
            sink.clone(),
        );
        let conversation = store.create();
        Self {
            queries,
            actions,
            backend,
            sink,
            store,
            engine,
            executor,
            conversation,
        }
    }

    fn scripted(provider: ProviderId, script: Vec<sentinel_agent::backend::ChatResponse>) -> Self {
        Self::new(ScriptedBackend::new(provider, script))
    }

    async fn send(&self, text: &str) -> sentinel_core::CoreResult<()> {
        self.engine
            .send_message(&self.conversation, text, CancellationToken::new())
            .await
    }

    fn proposed_plans(&self) -> Vec<Plan> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                AgentEvent::PlanProposed { plan } => Some(plan),
                _ => None,
            })
            .collect()
    }

    fn only_plan(&self) -> Plan {
        let plans = self.proposed_plans();
        assert_eq!(plans.len(), 1, "expected exactly one proposed plan");
        plans.into_iter().next().unwrap()
    }
}

fn read_then_kill_script() -> Vec<sentinel_agent::backend::ChatResponse> {
    vec![
        reply(vec![
            text("Checking live CPU usage."),
            tool(
                "r1",
                "list_processes",
                json!({"sort_by": "cpu", "limit": 5}),
            ),
        ]),
        reply(vec![
            text("hog (PID 4821) is using 97% CPU."),
            tool(
                "w1",
                "terminate_process",
                json!({"pid": HOG_PID, "reason": "97% CPU"}),
            ),
        ]),
        // A model that claims the action already happened changes nothing.
        reply(vec![text("I've killed hog for you. All done!")]),
    ]
}

fn approve_all(plan: &Plan) -> Vec<ActionDecision> {
    plan.actions
        .iter()
        .map(|a| ActionDecision {
            action_id: a.id.clone(),
            decision: Decision::Approve,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Invariant 1

#[tokio::test]
async fn invariant_1_write_calls_never_reach_the_committer_without_execute_plan() {
    let h = Harness::scripted(ProviderId::Anthropic, read_then_kill_script());
    h.send("what's eating my CPU right now and can you fix it")
        .await
        .unwrap();

    assert_eq!(
        h.actions.prepare_count(),
        1,
        "the write call is prepared as a preview"
    );
    assert_eq!(
        h.actions.commit_count(),
        0,
        "nothing executes during the turn"
    );
    assert!(
        h.queries.process_detail(HOG_PID).is_ok(),
        "process untouched"
    );
    let plan = h.only_plan();
    assert_eq!(plan.status, PlanStatus::AwaitingReview);
    assert!(matches!(plan.actions[0].state, PlanActionState::Pending));
    assert_eq!(
        plan.actions[0].preview.action,
        Action::TerminateProcess {
            target: sentinel_core::model::ProcessIdentity {
                pid: HOG_PID,
                start_time: 1_000
            }
        }
    );

    // More conversation without a decision still commits nothing.
    h.send("thanks").await.unwrap();
    assert_eq!(h.actions.commit_count(), 0);

    // The only route to execution is the executor with explicit approval.
    let done = h
        .executor
        .execute(&plan.id, &approve_all(&plan))
        .await
        .unwrap();
    assert_eq!(h.actions.commit_count(), 1);
    assert!(matches!(
        done.actions[0].state,
        PlanActionState::Succeeded { .. }
    ));
}

use sentinel_core::service::SystemQueries;

// ---------------------------------------------------------------------------------------------
// Invariant 2

#[tokio::test]
async fn invariant_2_write_without_prior_read_in_the_turn_is_refused() {
    let h = Harness::scripted(
        ProviderId::Openai,
        vec![
            reply(vec![tool(
                "w1",
                "terminate_process",
                json!({"pid": HOG_PID, "reason": "guess"}),
            )]),
            reply(vec![text("I could not stop it.")]),
        ],
    );
    h.send("kill the hog").await.unwrap();
    let results = h.backend.tool_results();
    assert_eq!(results.len(), 1);
    assert!(results[0].2, "refusal is a tool error");
    assert!(results[0].1.contains("Refused"), "{}", results[0].1);
    assert_eq!(h.actions.prepare_count(), 0);
    assert!(h.proposed_plans().is_empty());
}

#[tokio::test]
async fn invariant_2_read_in_the_same_response_does_not_ground_a_write() {
    let h = Harness::scripted(
        ProviderId::Gemini,
        vec![
            reply(vec![
                tool("r1", "list_processes", json!({})),
                tool(
                    "w1",
                    "force_kill_process",
                    json!({"pid": HOG_PID, "reason": "parallel guess"}),
                ),
            ]),
            // After seeing the read results, the same proposal is accepted.
            reply(vec![tool(
                "w2",
                "force_kill_process",
                json!({"pid": HOG_PID, "reason": "97% CPU, unresponsive"}),
            )]),
            reply(vec![text("Queued a force kill for your review.")]),
        ],
    );
    h.send("force kill whatever is hogging the CPU")
        .await
        .unwrap();
    let requests = h.backend.requests.lock().unwrap().clone();
    let first_round_results: Vec<_> = requests[1]
        .messages
        .last()
        .unwrap()
        .content
        .iter()
        .filter_map(|b| match b {
            sentinel_agent::backend::ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => Some((tool_use_id.clone(), *is_error)),
            _ => None,
        })
        .collect();
    assert_eq!(
        first_round_results,
        [("r1".to_owned(), false), ("w1".to_owned(), true)]
    );
    assert_eq!(h.actions.prepare_count(), 1);
    assert_eq!(h.actions.commit_count(), 0);
    assert_eq!(h.only_plan().actions.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Invariant 3

fn temp_files(count: usize) -> (std::path::PathBuf, Vec<String>) {
    let dir =
        std::env::temp_dir().join(format!("sentinel-safety-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let files = (0..count)
        .map(|i| {
            let path = dir.join(format!("cache-{i}.bin"));
            std::fs::write(&path, b"stale cache").unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    (dir, files)
}

#[tokio::test]
async fn invariant_3_blanket_cleanup_request_still_yields_a_plan_and_zero_commits() {
    let (dir, files) = temp_files(2);
    let sixty_days_ago = now_ms() / 1000 - 60 * 86_400;
    let script = vec![
        reply(vec![tool(
            "r1",
            "find_large_files",
            json!({"path": dir.to_string_lossy(), "min_size_mb": 0, "unused_for_days": 14}),
        )]),
        reply(vec![
            text("Two stale caches (10 GB) and a runaway process."),
            tool(
                "w1",
                "move_to_trash",
                json!({"paths": files.clone(), "reason": "unused for 60 days"}),
            ),
            tool(
                "w2",
                "terminate_process",
                json!({"pid": HOG_PID, "reason": "97% CPU"}),
            ),
        ]),
        reply(vec![text(
            "Done! Everything is cleaned up and your Mac is fast again.",
        )]),
    ];
    let h = Harness::scripted(ProviderId::OllamaCloud, script);
    *h.queries.files.lock().unwrap() = files
        .iter()
        .map(|p| FileEntry {
            node_id: 1,
            path: p.clone(),
            name: "cache.bin".into(),
            size_bytes: 5_000_000_000,
            modified: Some(sixty_days_ago),
            accessed: Some(sixty_days_ago),
            extension: Some("bin".into()),
            file_kind: FileKindGroup::Data,
        })
        .collect();

    h.send("Just clean up everything, don't ask me anything")
        .await
        .unwrap();

    let events = h.sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::PlanProposed { .. }))
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished { .. })
    ));
    assert_eq!(h.only_plan().actions.len(), 2);
    assert_eq!(h.actions.commit_count(), 0);
    for file in &files {
        assert!(
            std::path::Path::new(file).exists(),
            "{file} must not be touched"
        );
    }
    assert!(h.queries.process_detail(HOG_PID).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------------------------
// Invariant 4

async fn three_action_plan(provider: ProviderId) -> Harness {
    let h = Harness::scripted(
        provider,
        vec![
            reply(vec![tool("r1", "list_processes", json!({}))]),
            reply(vec![
                tool(
                    "w1",
                    "terminate_process",
                    json!({"pid": HOG_PID, "reason": "a"}),
                ),
                tool(
                    "w2",
                    "set_process_priority",
                    json!({"pid": 812, "nice": 10, "reason": "b"}),
                ),
                tool(
                    "w3",
                    "force_kill_process",
                    json!({"pid": 812, "reason": "c"}),
                ),
            ]),
            reply(vec![text("Three proposals ready.")]),
        ],
    );
    h.send("tidy up processes").await.unwrap();
    h
}

#[tokio::test]
async fn invariant_4_rejected_and_omitted_actions_are_never_committed() {
    let h = three_action_plan(ProviderId::Ollama).await;
    let plan = h.only_plan();
    assert_eq!(plan.actions.len(), 3);
    let decisions = vec![
        ActionDecision {
            action_id: plan.actions[0].id.clone(),
            decision: Decision::Approve,
        },
        ActionDecision {
            action_id: plan.actions[1].id.clone(),
            decision: Decision::Reject,
        },
        // actions[2] omitted
    ];
    let done = h.executor.execute(&plan.id, &decisions).await.unwrap();

    let committed = h.actions.committed.lock().unwrap().clone();
    assert_eq!(committed, vec![plan.actions[0].preview.action.clone()]);
    assert!(matches!(
        done.actions[0].state,
        PlanActionState::Succeeded { .. }
    ));
    assert!(matches!(done.actions[1].state, PlanActionState::Rejected));
    assert!(matches!(done.actions[2].state, PlanActionState::Rejected));
    let rejected = h.actions.rejected.lock().unwrap().clone();
    assert_eq!(
        rejected,
        vec![
            plan.actions[1].preview.token.clone(),
            plan.actions[2].preview.token.clone()
        ]
    );
    assert_eq!(done.status, PlanStatus::Completed);
    assert_eq!(h.store.plan(&plan.id).unwrap(), done);
}

#[tokio::test]
async fn invariant_4_decisions_for_unknown_actions_commit_nothing() {
    let h = three_action_plan(ProviderId::Anthropic).await;
    let plan = h.only_plan();
    let err = h
        .executor
        .execute(
            &plan.id,
            &[ActionDecision {
                action_id: "not-in-plan".into(),
                decision: Decision::Approve,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SentinelError::InvalidInput { .. }));
    assert_eq!(h.actions.commit_count(), 0);
}

// ---------------------------------------------------------------------------------------------
// Invariant 5

#[tokio::test]
async fn invariant_5_expired_tokens_are_not_committed_and_revise_re_prepares() {
    let h = Harness::scripted(ProviderId::Gemini, read_then_kill_script());
    *h.actions.issue_expired.lock().unwrap() = true;
    h.send("what's eating my CPU").await.unwrap();
    let plan = h.only_plan();

    // Direct commit of the expired token is refused by the pipeline.
    assert_eq!(
        h.actions
            .commit(&plan.actions[0].preview.token)
            .unwrap_err(),
        SentinelError::ActionTokenInvalid
    );

    // Revising re-prepares with a fresh token and puts the card back to Pending.
    *h.actions.issue_expired.lock().unwrap() = false;
    let revised = h
        .executor
        .revise(
            &plan.id,
            &plan.actions[0].id,
            plan.actions[0].preview.action.clone(),
        )
        .await
        .unwrap();
    assert!(matches!(revised.actions[0].state, PlanActionState::Pending));
    assert_ne!(
        revised.actions[0].preview.token,
        plan.actions[0].preview.token
    );
    let done = h
        .executor
        .execute(&plan.id, &approve_all(&revised))
        .await
        .unwrap();
    assert!(matches!(
        done.actions[0].state,
        PlanActionState::Succeeded { .. }
    ));
    assert_eq!(h.actions.commit_count(), 1);
}

#[tokio::test]
async fn invariant_5_expired_preview_approved_without_revision_becomes_expired() {
    let h = Harness::scripted(ProviderId::Openai, read_then_kill_script());
    *h.actions.issue_expired.lock().unwrap() = true;
    h.send("what's eating my CPU").await.unwrap();
    let plan = h.only_plan();
    let done = h
        .executor
        .execute(&plan.id, &approve_all(&plan))
        .await
        .unwrap();
    assert!(matches!(done.actions[0].state, PlanActionState::Expired));
    assert_eq!(h.actions.commit_count(), 0);
}

#[tokio::test]
async fn invariant_5_reused_tokens_fail_with_action_token_invalid() {
    let h = Harness::scripted(ProviderId::OllamaCloud, read_then_kill_script());
    h.send("what's eating my CPU").await.unwrap();
    let plan = h.only_plan();
    let token = plan.actions[0].preview.token.clone();

    // Token spent elsewhere (for example the same preview committed from the UI) …
    h.actions.commit(&token).unwrap();
    assert_eq!(
        h.actions.commit(&token).unwrap_err(),
        SentinelError::ActionTokenInvalid
    );
    // … so the plan cannot run it a second time.
    let done = h
        .executor
        .execute(&plan.id, &approve_all(&plan))
        .await
        .unwrap();
    assert!(matches!(done.actions[0].state, PlanActionState::Expired));
    assert_eq!(h.actions.commit_count(), 1);

    // A completed plan cannot be executed again.
    assert_eq!(
        h.executor
            .execute(&plan.id, &approve_all(&plan))
            .await
            .unwrap_err(),
        SentinelError::ActionTokenInvalid
    );
    assert_eq!(h.actions.commit_count(), 1);
}

// ---------------------------------------------------------------------------------------------
// Invariant 6

/// Provider-independent fingerprint of a turn plus execution.
fn fingerprint(h: &Harness) -> Vec<String> {
    h.sink
        .events()
        .into_iter()
        .map(|e| match e {
            AgentEvent::TurnStarted { model, .. } => format!("turn:{model}"),
            AgentEvent::TextDelta { text } => format!("text:{text}"),
            AgentEvent::ToolCallStarted {
                name,
                access,
                input,
                ..
            } => format!("call:{name}:{access:?}:{input}"),
            AgentEvent::ToolCallFinished { ok, summary, .. } => format!("done:{ok}:{summary}"),
            AgentEvent::PlanProposed { plan } | AgentEvent::PlanUpdated { plan } => format!(
                "plan:{:?}:{}",
                plan.status,
                plan.actions
                    .iter()
                    .map(|a| format!("{}={:?}", a.preview.title, std::mem::discriminant(&a.state)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            AgentEvent::TurnFinished { stop_reason } => format!("finish:{stop_reason:?}"),
            AgentEvent::Error { error } => format!("error:{}", error.message),
        })
        .collect()
}

#[tokio::test]
async fn invariant_6_identical_behaviour_for_every_provider() {
    let mut runs = Vec::new();
    for provider in ProviderId::ALL {
        let mut script = vec![reply(vec![tool(
            "w0",
            "terminate_process",
            json!({"pid": HOG_PID, "reason": "ungrounded"}),
        )])];
        script.extend(read_then_kill_script());
        let h = Harness::scripted(provider, script);
        h.send("what's eating my CPU right now and can you fix it")
            .await
            .unwrap();
        let plan = h.only_plan();
        h.executor
            .execute(&plan.id, &approve_all(&plan))
            .await
            .unwrap();
        let prepared = h.actions.prepared.lock().unwrap().clone();
        assert!(
            matches!(&prepared[0].1, sentinel_core::action::Origin::Agent { provider: p, model, request, .. }
            if p == provider.display_name() && model == "test-model" && request.contains("eating my CPU"))
        );
        runs.push((
            provider,
            fingerprint(&h),
            h.actions.prepare_count(),
            h.actions.commit_count(),
        ));
    }
    let (_, reference, prepares, commits) = &runs[0];
    assert_eq!((*prepares, *commits), (1, 1));
    for (provider, events, p, c) in &runs[1..] {
        assert_eq!(events, reference, "{provider:?} diverged");
        assert_eq!((p, c), (prepares, commits), "{provider:?} diverged");
    }
}

// ---------------------------------------------------------------------------------------------
// Additional guarantees

#[tokio::test]
async fn write_tool_results_never_claim_success() {
    let h = Harness::scripted(ProviderId::Anthropic, read_then_kill_script());
    h.send("fix my CPU").await.unwrap();
    let results = h.backend.tool_results();
    let (_, content, is_error) = results
        .iter()
        .find(|(id, _, _)| id == "w1")
        .expect("write result");
    assert!(!is_error);
    let value: Value = serde_json::from_str(content).unwrap();
    assert_eq!(value["executed"], false);
    assert_eq!(value["status"], "queued_for_user_review");
    assert!(content.contains("NOT been performed"), "{content}");
    for (_, content, _) in &results {
        let lower = content.to_lowercase();
        assert!(!lower.contains("\"executed\":true"));
        assert!(
            !lower.contains("succeeded") && !lower.contains("successfully"),
            "{content}"
        );
    }
    let summaries: Vec<String> = h
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCallFinished { summary, .. } => Some(summary),
            _ => None,
        })
        .collect();
    assert!(
        summaries
            .iter()
            .any(|s| s.starts_with("Proposed for your review")),
        "{summaries:?}"
    );
}

#[tokio::test]
async fn unknown_and_disguised_tools_are_rejected() {
    let h = Harness::scripted(
        ProviderId::Ollama,
        vec![
            reply(vec![tool("r1", "list_processes", json!({}))]),
            reply(vec![
                tool(
                    "x1",
                    "Terminate_Process",
                    json!({"pid": HOG_PID, "reason": "case"}),
                ),
                tool(
                    "x2",
                    "terminate_process ",
                    json!({"pid": HOG_PID, "reason": "space"}),
                ),
                tool("x3", "list_processes_and_kill", json!({"pid": HOG_PID})),
                tool("x4", "get_system_overview", json!({"kill_pid": HOG_PID})),
                tool(
                    "x5",
                    "list_processes",
                    json!({"terminate": true, "pid": HOG_PID}),
                ),
                tool("x6", "shell", json!({"command": "kill -9 4821"})),
                tool(
                    "x7",
                    "terminate_process",
                    Value::String("{\"pid\": 4821".into()),
                ),
            ]),
            reply(vec![text("Nothing could be done.")]),
        ],
    );
    h.send("kill the hog by any means").await.unwrap();
    let results = h.backend.tool_results();
    for id in ["x1", "x2", "x3", "x4", "x5", "x6", "x7"] {
        let (_, content, is_error) = results.iter().find(|(r, _, _)| r == id).unwrap();
        assert!(*is_error, "{id} must be rejected: {content}");
    }
    assert_eq!(h.actions.prepare_count(), 0);
    assert_eq!(h.actions.commit_count(), 0);
    assert!(h.proposed_plans().is_empty());
    assert!(h.queries.process_detail(HOG_PID).is_ok());
}

#[test]
fn engine_source_never_mentions_the_committer() {
    let source = include_str!("../src/engine.rs");
    for forbidden in ["ActionCommitter", ".commit(", ".reject(", "PlanExecutor"] {
        assert!(
            !source.contains(forbidden),
            "engine.rs must not reference `{forbidden}`"
        );
    }
}

#[tokio::test]
async fn cancellation_ends_the_turn_without_commits() {
    let backend = ScriptedBackend::with(
        ProviderId::Anthropic,
        read_then_kill_script().into_iter().map(Ok).collect(),
        true,
        Duration::from_secs(5),
    );
    let h = Harness::new(backend);
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
    });
    let started = std::time::Instant::now();
    let result = h
        .engine
        .send_message(&h.conversation, "fix my CPU", cancel)
        .await;
    assert_eq!(result.unwrap_err(), SentinelError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        matches!(h.sink.events().last(), Some(AgentEvent::TurnFinished { stop_reason: StopReason::Other { raw } }) if raw == "cancelled")
    );
    assert_eq!(h.actions.commit_count(), 0);
}

#[tokio::test]
async fn models_without_tool_support_get_a_specific_error() {
    let backend = ScriptedBackend::with(ProviderId::Ollama, vec![], false, Duration::ZERO);
    let h = Harness::new(backend);
    let err = h.send("what's eating my CPU").await.unwrap_err();
    assert!(
        matches!(&err, SentinelError::Unavailable { feature, .. } if feature == "tool calling")
    );
    assert!(
        h.sink
            .events()
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { .. }))
    );
    assert!(h.backend.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn follow_up_turn_reports_real_outcomes() {
    let mut script = read_then_kill_script();
    script.push(reply(vec![text("Stopped hog: CPU went from 90% to 20%.")]));
    let h = Harness::scripted(ProviderId::Gemini, script);
    h.send("what's eating my CPU").await.unwrap();
    let plan = h.only_plan();
    let done = h
        .executor
        .execute(&plan.id, &approve_all(&plan))
        .await
        .unwrap();
    h.engine
        .report_execution(&done, CancellationToken::new())
        .await
        .unwrap();

    let last = h.backend.requests.lock().unwrap().last().cloned().unwrap();
    let report = last
        .messages
        .iter()
        .rev()
        .find_map(|m| {
            m.content.iter().find_map(|b| match b {
                sentinel_agent::backend::ContentBlock::Text { text }
                    if text.contains("real results") =>
                {
                    Some(text.clone())
                }
                _ => None,
            })
        })
        .expect("outcome report sent to the model");
    assert!(report.contains("Quit PID 4821 done"), "{report}");
    assert!(
        report.contains("\"value\": 90.0") && report.contains("\"value\": 20.0"),
        "{report}"
    );
    assert_eq!(
        h.actions.commit_count(),
        1,
        "the report turn commits nothing further"
    );
}

#[tokio::test]
async fn edits_can_only_narrow_a_proposal() {
    let (dir, files) = temp_files(3);
    let h = Harness::scripted(
        ProviderId::Openai,
        vec![
            reply(vec![tool("r1", "list_volumes", json!({}))]),
            reply(vec![tool(
                "w1",
                "move_to_trash",
                json!({"paths": files.clone(), "reason": "old caches"}),
            )]),
            reply(vec![text("Proposed.")]),
        ],
    );
    h.send("trash old caches").await.unwrap();
    let plan = h.only_plan();
    let action_id = plan.actions[0].id.clone();

    let widened = Action::TrashPaths {
        paths: vec![files[0].clone(), "/etc/hosts".into()],
    };
    assert!(
        h.executor
            .revise(&plan.id, &action_id, widened)
            .await
            .is_err()
    );

    let narrowed = h
        .executor
        .revise(
            &plan.id,
            &action_id,
            Action::TrashPaths {
                paths: vec![files[0].clone()],
            },
        )
        .await
        .unwrap();
    assert_eq!(
        narrowed.actions[0].preview.action,
        Action::TrashPaths {
            paths: vec![files[0].clone()]
        }
    );
    assert_eq!(
        h.actions.rejected.lock().unwrap().len(),
        1,
        "superseded token released"
    );
    assert_eq!(h.actions.commit_count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}
