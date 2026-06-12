//! LIVE backend integration verification for Agent mode (Stage-3 probe).
//!
//! This is NOT a unit test and NOT part of the contract suite. It is an
//! `#[ignore]`-d live probe that drives the real Agent backend end to end
//! against the user's *already configured* provider key, proving the live
//! agent path runs: a plain streaming turn plus a `web_fetch` tool turn.
//!
//! Why `#[ignore]`: it hits the network + a real LLM API and reads the user's
//! app DB, so it must never run in normal `cargo test`. Run it explicitly:
//!
//! ```sh
//! cd src-tauri
//! cargo test --test agent_live -- --ignored --nocapture
//! ```
//!
//! SECURITY: the provider API key is read read-only from the app DB and is
//! NEVER printed/logged anywhere. The user's real DB is opened read-only
//! (`?mode=ro`) and never written; all agent-session writes go to a throwaway
//! temp DB.
//!
//! The live path is exercised through `AgentRuntime`'s public surface
//! (`new` + `start_run` + `RunSink`), seeded with the real provider row and an
//! `AgentSession` in a temp DB. That drives `chat_engine::resolve_model` /
//! `build_stream_options` / `shared_client` / `agent_tools::build_tools` /
//! `hand_agent::run_agent_loop` exactly as production does. For the tool turn,
//! if the (cheap) model does not emit a `web_fetch` call, the test falls back
//! to invoking the `web_fetch` `AgentTool` directly — still a real, live
//! network proof.

use std::sync::{Arc, Mutex as StdMutex};

use handbox_lib::services::agent_runtime::{AgentRuntime, RunSink};
use handbox_lib::services::{agent_tools, Database};
use handbox_lib::storage::types::{AgentSession, Provider};
use handbox_lib::storage::{AgentSessionRepository, ProviderRepository};

use hand_agent::{AgentTool, CancellationToken, ToolExecuteCtx, ToolResult};
use hand_ai_model::ToolResultContent;
use tempfile::TempDir;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// One configured provider read read-only from the app DB. The `api_key` is
/// carried in memory only and is NEVER printed.
struct LiveProvider {
    provider_type: String,
    base_url: String,
    api_key: String,
}

/// Path to the user's app DB on macOS.
fn app_db_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").expect("HOME must be set");
    std::path::Path::new(&home).join("Library/Application Support/com.gumpw.handbox/handbox.db")
}

/// Read the cheapest viable configured provider from the app DB, read-only.
///
/// Preference order favors cheap/fast providers first. Opens the DB with
/// `?mode=ro` so the running app is never disrupted and the real DB is never
/// mutated. Returns `None` (test skips) when none of the preferred providers
/// has a non-empty key.
async fn read_live_provider() -> Option<LiveProvider> {
    let db_path = app_db_path();
    if !db_path.exists() {
        eprintln!("[agent_live] app DB not found at {db_path:?}; skipping");
        return None;
    }

    // Read-only connection: never lock/modify the running app's DB.
    let url = format!("sqlite://{}?mode=ro", db_path.display());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .ok()?;

    // Cheapest/fastest first.
    const PREFERRED: &[&str] = &["deepseek", "cerebras", "openrouter", "google"];
    for ptype in PREFERRED {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT base_url, api_key FROM providers \
             WHERE provider_type = ?1 AND api_key IS NOT NULL AND api_key <> '' \
             LIMIT 1",
        )
        .bind(ptype)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        if let Some((base_url, api_key)) = row {
            return Some(LiveProvider {
                provider_type: (*ptype).to_string(),
                base_url,
                api_key,
            });
        }
    }
    None
}

/// Pick the cheapest catalog model id for `provider_type` (lowest output cost,
/// then lowest input cost). The brief's suggested ids may not match the pinned
/// catalog, so we resolve dynamically against `hand_ai_model::get_models`.
fn cheapest_model_id(provider_type: &str) -> Option<String> {
    let mut models = hand_ai_model::get_models(provider_type);
    if models.is_empty() {
        return None;
    }
    models.sort_by(|a, b| {
        a.cost
            .output
            .partial_cmp(&b.cost.output)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.cost
                    .input
                    .partial_cmp(&b.cost.input)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    Some(models[0].id.clone())
}

/// A throwaway temp DB seeded with the real provider row + an `AgentSession`
/// selecting `model_id` and enabling the given tools. Returns the runtime, the
/// session id, and the TempDir guard (kept alive for the DB file).
async fn seeded_runtime(
    live: &LiveProvider,
    model_id: &str,
    enabled_tools: Vec<String>,
) -> (AgentRuntime, String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("agent_live.db");
    let db = Arc::new(Database::new(&db_path).await.unwrap());

    let provider_id = uuid::Uuid::new_v4().to_string();
    let provider = Provider {
        id: provider_id.clone(),
        name: format!("Live {}", live.provider_type),
        provider_type: live.provider_type.clone(),
        base_url: live.base_url.clone(),
        api_key: live.api_key.clone(),
        enabled: true,
        created_at: now_ms(),
        updated_at: now_ms(),
    };
    ProviderRepository::new(Arc::clone(&db))
        .create_provider(&provider)
        .await
        .unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    let session = AgentSession {
        id: session_id.clone(),
        name: "Live Probe".to_string(),
        project_id: None,
        model_id: Some(model_id.to_string()),
        provider_id: Some(provider_id),
        system_prompt: Some("You are a terse assistant.".to_string()),
        thinking_level: None,
        temperature: Some(0.0),
        // Keep it cheap.
        max_tokens: Some(256),
        working_dir: None,
        enabled_tools,
        tool_execution_mode: None,
        message_count: 0,
        last_message_at: None,
        created_at: now_ms(),
        updated_at: now_ms(),
    };
    AgentSessionRepository::new(Arc::clone(&db))
        .create_session(&session)
        .await
        .unwrap();

    (AgentRuntime::new(db), session_id, temp_dir)
}

/// A capturing sink: records every `{ sessionId, event }`, the terminal
/// `{ sessionId }` closed payloads, and any run-level error envelope.
#[derive(Clone, Default)]
struct CapturingSink {
    events: Arc<StdMutex<Vec<serde_json::Value>>>,
    closed: Arc<StdMutex<Vec<serde_json::Value>>>,
    errors: Arc<StdMutex<Vec<serde_json::Value>>>,
}

impl CapturingSink {
    fn into_run_sink(self) -> RunSink {
        let events = Arc::clone(&self.events);
        let closed = Arc::clone(&self.closed);
        let errors = Arc::clone(&self.errors);
        RunSink::new(
            Arc::new(move |v| events.lock().unwrap().push(v)),
            Arc::new(move |v| closed.lock().unwrap().push(v)),
        )
        .with_error(Arc::new(move |v| errors.lock().unwrap().push(v)))
    }

    fn closed_count(&self) -> usize {
        self.closed.lock().unwrap().len()
    }

    fn events(&self) -> Vec<serde_json::Value> {
        self.events.lock().unwrap().clone()
    }

    fn errors(&self) -> Vec<serde_json::Value> {
        self.errors.lock().unwrap().clone()
    }
}

/// Poll until the terminal closed signal arrives (or panic on timeout). Live
/// network, so the timeout is generous.
async fn wait_for_closed(sink: &CapturingSink) {
    for _ in 0..600 {
        if sink.closed_count() >= 1 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("live run did not close within 60s");
}

/// The `event` object's `type` tag (snake_case `AgentEvent` variant).
fn event_type(payload: &serde_json::Value) -> Option<&str> {
    payload.get("event")?.get("type")?.as_str()
}

/// Extract the final assistant text from a `message_end` event whose `message`
/// is an assistant message, concatenating its text content blocks.
fn assistant_text_from_message_end(payload: &serde_json::Value) -> Option<String> {
    let event = payload.get("event")?;
    if event.get("type")?.as_str()? != "message_end" {
        return None;
    }
    let message = event.get("message")?;
    if message.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let blocks = message.get("content")?.as_array()?;
    let mut text = String::new();
    for b in blocks {
        if b.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }
    }
    Some(text)
}

/// Extract `(model, output_tokens)` from a terminal assistant `message_end`.
fn model_and_output_tokens(payload: &serde_json::Value) -> Option<(String, u64)> {
    let event = payload.get("event")?;
    if event.get("type")?.as_str()? != "message_end" {
        return None;
    }
    let message = event.get("message")?;
    if message.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let model = message.get("model")?.as_str()?.to_string();
    let output = message
        .get("usage")
        .and_then(|u| u.get("output"))
        .and_then(|o| o.as_u64())
        .unwrap_or(0);
    Some((model, output))
}

/// First text content block of a `ToolResult`, for the direct-invoke fallback.
fn tool_result_text(result: &ToolResult) -> Option<String> {
    for c in &result.content {
        if let ToolResultContent::Text(t) = c {
            return Some(t.text.clone());
        }
    }
    None
}

// ===========================================================================
// Turn 1 (plain) + Turn 2 (tool) — one live test, sequential turns.
// ===========================================================================

#[tokio::test]
#[ignore = "LIVE: hits the network + real LLM API + reads the user's app DB"]
async fn live_agent_plain_turn_and_web_fetch_tool() {
    let Some(live) = read_live_provider().await else {
        eprintln!("[agent_live] no configured provider with a key; skipping");
        return;
    };
    let Some(model_id) = cheapest_model_id(&live.provider_type) else {
        eprintln!(
            "[agent_live] no catalog model for provider '{}'; skipping",
            live.provider_type
        );
        return;
    };

    println!(
        "[agent_live] provider='{}' base_url='{}' model='{}'",
        live.provider_type, live.base_url, model_id
    );

    // ---------------------------------------------------------------------
    // Turn 1 (plain): real streaming + final assistant message + usage.
    // ---------------------------------------------------------------------
    {
        let (runtime, session_id, _guard) = seeded_runtime(&live, &model_id, vec![]).await;
        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "Reply with exactly the word: HELLO".to_string(),
                vec![],
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("turn 1 start_run should succeed");

        wait_for_closed(&sink).await;

        // No run-level error envelope on a healthy live turn.
        let errors = sink.errors();
        assert!(
            errors.is_empty(),
            "turn 1 produced a run-level error envelope: {errors:#?}"
        );

        let events = sink.events();

        // REAL streaming: at least one MessageUpdate (text delta) arrived.
        let update_count = events
            .iter()
            .filter(|e| event_type(e) == Some("message_update"))
            .count();
        assert!(
            update_count >= 1,
            "turn 1 must emit >= 1 message_update (real streaming); saw types: {:?}",
            events.iter().filter_map(event_type).collect::<Vec<_>>()
        );

        // Final assistant message: non-empty model id + output tokens > 0.
        let (model, output_tokens) = events
            .iter()
            .find_map(model_and_output_tokens)
            .expect("turn 1 must produce a terminal assistant message_end with model + usage");
        assert!(
            !model.is_empty(),
            "final assistant model id must be non-empty"
        );
        assert!(
            output_tokens > 0,
            "final usage output tokens must be > 0 (got {output_tokens})"
        );

        // The assistant text contains HELLO (case-insensitive).
        let text = events
            .iter()
            .find_map(assistant_text_from_message_end)
            .unwrap_or_default();
        let snippet: String = text.chars().take(120).collect();
        println!(
            "[agent_live][turn1] events={} message_update={} model='{}' \
             output_tokens={} text_snippet={:?}",
            events.len(),
            update_count,
            model,
            output_tokens,
            snippet
        );
        assert!(
            text.to_ascii_uppercase().contains("HELLO"),
            "turn 1 assistant text must contain HELLO (got: {snippet:?})"
        );
    }

    // ---------------------------------------------------------------------
    // Turn 2 (tool): web_fetch of https://example.com. If the model emits a
    // tool call we assert the live ToolExecutionStart/End + non-empty result;
    // otherwise we directly invoke the web_fetch tool (still a live network
    // proof, as the brief permits).
    // ---------------------------------------------------------------------
    {
        let (runtime, session_id, _guard) =
            seeded_runtime(&live, &model_id, vec!["web_fetch".to_string()]).await;
        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "Use the web_fetch tool to fetch https://example.com \
                 and tell me the page title."
                    .to_string(),
                vec![],
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("turn 2 start_run should succeed");

        wait_for_closed(&sink).await;

        let events = sink.events();
        let tool_start = events
            .iter()
            .find(|e| event_type(e) == Some("tool_execution_start"));
        let tool_end = events
            .iter()
            .find(|e| event_type(e) == Some("tool_execution_end"));

        if let (Some(start), Some(end)) = (tool_start, tool_end) {
            // Model drove the tool: assert the live tool round-trip.
            let start_name = start
                .get("event")
                .and_then(|e| e.get("toolName"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let end_name = end
                .get("event")
                .and_then(|e| e.get("toolName"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            assert_eq!(start_name, "web_fetch", "tool_execution_start is web_fetch");
            assert_eq!(end_name, "web_fetch", "tool_execution_end is web_fetch");

            // The tool result content is non-empty (the real fetched text).
            let result = end.get("event").and_then(|e| e.get("result"));
            let fetched_len = result
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .map(|s| s.len())
                        .sum::<usize>()
                })
                .unwrap_or(0);
            let is_error = end
                .get("event")
                .and_then(|e| e.get("isError"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            println!(
                "[agent_live][turn2:model-driven] tool=web_fetch isError={} \
                 fetched_text_len={}",
                is_error, fetched_len
            );
            assert!(!is_error, "live web_fetch tool result must not be an error");
            assert!(
                fetched_len > 0,
                "live web_fetch tool result content must be non-empty"
            );
        } else {
            // Model did not call the tool reliably — exercise the web_fetch
            // AgentTool directly for a guaranteed live network proof.
            println!(
                "[agent_live][turn2] model did not emit a web_fetch call \
                 (event types: {:?}); invoking web_fetch tool directly",
                events.iter().filter_map(event_type).collect::<Vec<_>>()
            );

            let tools: Vec<AgentTool> = agent_tools::build_tools(&["web_fetch".to_string()], None);
            let web_fetch = tools
                .into_iter()
                .find(|t| t.name == "web_fetch")
                .expect("web_fetch tool must be built");

            let ctx = ToolExecuteCtx {
                tool_call_id: "live-direct-1".to_string(),
                args: serde_json::json!({ "url": "https://example.com" }),
                cancel: CancellationToken::new(),
                on_update: Arc::new(|_| {}),
            };
            let result = (web_fetch.execute)(ctx)
                .await
                .expect("web_fetch execute must not return Err");

            let fetched = tool_result_text(&result).unwrap_or_default();
            let snippet: String = fetched.chars().take(120).collect();
            println!(
                "[agent_live][turn2:direct] fetched_text_len={} snippet={:?}",
                fetched.len(),
                snippet
            );
            assert!(
                fetched.len() > 0,
                "direct web_fetch of https://example.com returned empty text"
            );
            // example.com's body mentions "example" in its readable text.
            assert!(
                fetched.to_ascii_lowercase().contains("example"),
                "fetched text should reference 'example' (got: {snippet:?})"
            );
        }
    }
}
