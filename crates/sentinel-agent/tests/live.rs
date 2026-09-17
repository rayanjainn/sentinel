//! Live provider tests. All are `#[ignore]`d: they need a running local Ollama with `llama3.1:8b`,
//! or real API keys supplied through the environment (never committed):
//!
//! ```sh
//! cargo test -p sentinel-agent --test live -- --ignored --nocapture
//! SENTINEL_TEST_GEMINI_KEY=... cargo test -p sentinel-agent --test live gemini -- --ignored
//! SENTINEL_TEST_OLLAMA_CLOUD_KEY=... cargo test -p sentinel-agent --test live ollama_cloud -- --ignored
//! ```
//!
//! Optional model overrides: `SENTINEL_TEST_GEMINI_MODEL`, `SENTINEL_TEST_OLLAMA_CLOUD_MODEL`,
//! `SENTINEL_TEST_OLLAMA_MODEL`, `SENTINEL_TEST_ANTHROPIC_MODEL`, `SENTINEL_TEST_OPENAI_MODEL`.
//! System state is a test double; the model, the adapter and the wire traffic are real.

mod support;

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use sentinel_agent::backend::{AgentBackend, ChatMessage, ChatRequest, ContentBlock, Role};
use sentinel_agent::conversation::ConversationStore;
use sentinel_agent::engine::{AgentEngine, EngineConfig, EngineParts};
use sentinel_agent::events::AgentEvent;
use sentinel_agent::providers::catalog;
use sentinel_agent::providers::ollama::OllamaBackend;
use sentinel_agent::settings::{AgentSettings, ProviderId};
use sentinel_agent::tools::read::SystemReadTools;
use sentinel_agent::tools::specs;
use sentinel_agent::tools::write::SystemWriteTranslator;
use sentinel_core::action::ActionPreparer;
use support::*;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Runs "what's eating my CPU" through the real backend against a fake machine with one hog.
async fn full_turn(backend: Arc<dyn AgentBackend>, model: &str) {
    let queries = FakeQueries::with_processes(vec![
        process(4821, "render-hog", 97.4, 1_000),
        process(812, "postgres", 1.2, 2_000),
        process(990, "Finder", 0.3, 3_000),
    ]);
    let actions = FakeActions::new();
    let sink = RecordingSink::new();
    let store = ConversationStore::new();
    let engine = AgentEngine::new(EngineParts {
        backend,
        reads: Arc::new(SystemReadTools::new(queries.clone())),
        writes: Arc::new(SystemWriteTranslator::new(queries.clone())),
        preparer: Arc::clone(&actions) as Arc<dyn ActionPreparer>,
        store: Arc::clone(&store),
        sink: sink.clone(),
        config: EngineConfig {
            model: model.to_owned(),
            max_tool_rounds: 6,
            max_tokens: 4096,
        },
    });
    let conversation = store.create();
    engine
        .send_message(
            &conversation,
            "What's eating my CPU right now, and can you fix it?",
            CancellationToken::new(),
        )
        .await
        .expect("turn completes");

    let events = sink.events();
    for event in &events {
        match event {
            AgentEvent::ToolCallStarted { name, input, .. } => {
                eprintln!("tool call: {name} {input}")
            }
            AgentEvent::ToolCallFinished { ok, summary, .. } => {
                eprintln!("  -> ok={ok}: {summary}")
            }
            AgentEvent::PlanProposed { plan } => eprintln!(
                "plan: {}",
                plan.actions
                    .iter()
                    .map(|a| a.preview.title.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            AgentEvent::Error { error } => eprintln!("error: {}", error.message),
            _ => {}
        }
    }
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    eprintln!("assistant: {text}");

    let read_ok = events.iter().any(|e| matches!(e, AgentEvent::ToolCallFinished { ok: true, .. }))
        && events.iter().any(|e| matches!(e, AgentEvent::ToolCallStarted { name, .. }
            if sentinel_agent::tools::access_of(name) == Some(sentinel_agent::tools::ToolAccess::Read)));
    assert!(read_ok, "the model should read live state before answering");
    assert_eq!(actions.commit_count(), 0, "a turn never commits");
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished { .. })
    ));
}

/// One non-streaming request with tools, to prove the request shape is accepted.
async fn single_tool_request(backend: &dyn AgentBackend, model: &str) {
    let response = backend
        .send_message(&ChatRequest {
            model: model.to_owned(),
            system: "You are a system monitor. Always call a tool to answer.".into(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "List the mounted volumes.".into(),
                }],
            }],
            tools: specs::definitions(),
            max_tokens: 1024,
            temperature: None,
        })
        .await
        .expect("request accepted");
    assert!(
        response
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. })),
        "expected a tool call, got {:?}",
        response.content
    );
}

#[tokio::test]
#[ignore = "needs local Ollama with llama3.1:8b"]
async fn local_ollama_full_turn() {
    let model = env("SENTINEL_TEST_OLLAMA_MODEL").unwrap_or_else(|| "llama3.1:8b".into());
    let settings = AgentSettings::default();
    let backend = catalog::prepare_backend(ProviderId::Ollama, None, &settings, &model)
        .await
        .expect("Ollama running with the model pulled");
    assert!(backend.supports_tool_calling(&model));
    let local = OllamaBackend::local(settings.ollama_base_url.clone());
    assert!(!local.version().await.unwrap().is_empty());
    assert!(
        local
            .list_models()
            .await
            .unwrap()
            .iter()
            .any(|m| m.id == model && m.supports_tools)
    );
    full_turn(backend, &model).await;
}

async fn cloud(provider: ProviderId, key_var: &str, model_var: &str) {
    let Some(key) = env(key_var) else {
        eprintln!("skipped: set {key_var} to run this test");
        return;
    };
    let settings = AgentSettings::default();
    let model = env(model_var).unwrap_or_else(|| catalog::default_model(provider));

    let bad =
        catalog::build_backend(provider, Some("sentinel-invalid-key".into()), &settings).unwrap();
    let rejected = bad
        .validate_credentials()
        .await
        .expect_err("bogus key must be rejected");
    assert!(
        sentinel_agent::providers::http::is_auth_rejection(&rejected),
        "{rejected:?}"
    );

    let backend = catalog::prepare_backend(provider, Some(key), &settings, &model)
        .await
        .expect("backend");
    backend.validate_credentials().await.expect("key accepted");
    let models = backend.list_models().await.expect("model list");
    assert!(!models.is_empty());
    eprintln!(
        "{} models, e.g. {:?}",
        models.len(),
        models.iter().take(5).map(|m| &m.id).collect::<Vec<_>>()
    );
    single_tool_request(backend.as_ref(), &model).await;
    full_turn(backend, &model).await;
}

#[tokio::test]
#[ignore = "needs SENTINEL_TEST_GEMINI_KEY"]
async fn gemini_live() {
    cloud(
        ProviderId::Gemini,
        "SENTINEL_TEST_GEMINI_KEY",
        "SENTINEL_TEST_GEMINI_MODEL",
    )
    .await;
}

#[tokio::test]
#[ignore = "needs SENTINEL_TEST_OLLAMA_CLOUD_KEY"]
async fn ollama_cloud_live() {
    cloud(
        ProviderId::OllamaCloud,
        "SENTINEL_TEST_OLLAMA_CLOUD_KEY",
        "SENTINEL_TEST_OLLAMA_CLOUD_MODEL",
    )
    .await;
}

#[tokio::test]
#[ignore = "needs SENTINEL_TEST_ANTHROPIC_KEY"]
async fn anthropic_live() {
    cloud(
        ProviderId::Anthropic,
        "SENTINEL_TEST_ANTHROPIC_KEY",
        "SENTINEL_TEST_ANTHROPIC_MODEL",
    )
    .await;
}

#[tokio::test]
#[ignore = "needs SENTINEL_TEST_OPENAI_KEY"]
async fn openai_live() {
    cloud(
        ProviderId::Openai,
        "SENTINEL_TEST_OPENAI_KEY",
        "SENTINEL_TEST_OPENAI_MODEL",
    )
    .await;
}

#[test]
fn live_tests_never_embed_keys() {
    let source = include_str!("live.rs");
    let marker = ["sk-", "ant-"].concat();
    assert!(!source.contains(&marker));
    assert!(json!({"k": env("SENTINEL_TEST_NONEXISTENT")})["k"].is_null());
}
