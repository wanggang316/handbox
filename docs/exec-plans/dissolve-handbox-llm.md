# ExecPlan: Dissolve the handbox-llm crate

**Status:** Approved
**Author:** Claude (HandBox side)
**Date:** 2026-05-25
**Approved by:** Gump, 2026-05-25

This is a living document. The Progress, Surprises & Discoveries, Decision Log, and Outcomes & Retrospective sections must be kept up to date as work proceeds.

## Purpose

The HandBox app currently funnels every chat and model-list operation through `handbox-llm`, an internal Rust crate that was built when HandBox needed to unify four hand-coded provider adapters. As of `model-v0.2.0` (commit `cfc813c`), hand-ai already provides exactly that unification for 30+ providers — the request/event model, the per-protocol adapter, the cancellation gate, and the capability metadata are all in `hand_ai_model`. `handbox-llm` has degenerated into a translation shim plus a handful of small leaf types (`LlmMessageRole`, `LlmToolCall`, `LlmToolFunction`, etc.) that have leaked into HandBox's storage and IPC layer.

After this plan lands, **the `handbox-llm` crate no longer exists**. Chat and model code in `src-tauri/src/services/` calls `hand_ai_model::Client` directly. The few leaf types that leak into HandBox's persistence layer (`storage/types/*`) live in `src-tauri/src/models/llm_types.rs` as plain HandBox structs. A reader auditing HandBox's LLM integration sees one vendor (hand-ai) and one set of types, not two.

User-observable behavior does not change. The picker still shows the same 30+ providers. Chat still works the same way for OpenAI, Anthropic, Google, OpenRouter, Deepseek. New providers (Groq, Mistral, xAI, etc.) still work via the catalog path. What changes is **internal**: ~3000 fewer lines of Rust, one fewer trait layer between the chat handler and the network, and no second type vocabulary to keep in sync.

## Progress

**State:** Running
**Active worker:** none (M1-T2 complete; M1-T4 about to dispatch)
**Last handoff:** 2026-05-25T20:35Z — M1-T2 — completed

### Handoff log

2026-05-25T19:20Z  M1-T1  implementer        DONE             83d28a0
2026-05-25T19:24Z  M1-T1  spec-reviewer      compliant
2026-05-25T19:28Z  M1-T1  code-reviewer      approve
2026-05-25T19:30Z  M1-T1  user-test-validator vacuous-pass (no cases bound)
2026-05-25T20:08Z  M1-T2  implementer        BLOCKED          (nominal-type heavy-aggregate seams)
2026-05-25T20:11Z  M1-T2  controller         replan           Decision Log: switch to pub-use re-export strategy + add M3-T0
2026-05-25T20:25Z  M1-T2  implementer        DONE             b758f56
2026-05-25T20:28Z  M1-T2  spec-reviewer      compliant
2026-05-25T20:34Z  M1-T2  code-reviewer      approve          (2 suggestions deferred to M2-T2 same-file touches)
2026-05-25T20:35Z  M1-T2  user-test-validator structural-pass UT-DISSOLVE-004 (pub-use preserves nominal identity & serde shape)
2026-05-25T20:48Z  M1-T4  implementer        DONE             21e038b
2026-05-25T20:52Z  M1-T4  spec-reviewer      compliant
2026-05-25T20:55Z  M1-T4  code-reviewer      approve          (1 suggestion / 1 nit / 1 FYI — cosmetic)
2026-05-25T20:56Z  M1-T4  user-test-validator vacuous-pass (static gate, no cases)
2026-05-25T20:57Z  M1     exit-gate          PASS             cargo check clean both default + --no-default-features; storage::types 6/6 green; new serde-repr test green; pre-existing 9 unrelated failures unchanged
2026-05-25T21:35Z  M2-T1  implementer        DONE_W_CONCERNS  f068875 (4 observation-class concerns accepted)
2026-05-25T21:40Z  M2-T1  spec-reviewer      compliant
2026-05-25T21:48Z  M2-T1  code-reviewer      approve-with-fixes  (2 Important: auth-error mapping; test density gap; +1 clippy CI hint)
2026-05-25T22:05Z  M2-T1.1 implementer       DONE             a512c4f  (3 fixes landed; 10/10 chat_engine tests; 87 lib pass)
2026-05-25T22:06Z  M2-T1  user-test-validator structural-pass UT-DISSOLVE-002 + UT-003 (no caller wired yet; chat_engine module ready for M2-T2 to dispatch)
2026-05-26T01:00Z  M2-T2  controller         BLOCKED-then-mid-flight-then-revert  (decision-log entry 2026-05-26 captures the split rationale)
2026-05-26T01:30Z  M2-T2a implementer        DONE_W_CONCERNS  ba887a8 (4 observation-class concerns; #1 escalated by code-reviewer)
2026-05-26T01:40Z  M2-T2a spec-reviewer      compliant
2026-05-26T01:50Z  M2-T2a code-reviewer      approve-with-fixes  (2 Important: model_id revert + expect-to-internal_error)
2026-05-26T02:00Z  M2-T2a.1 implementer      DONE             77ab4ef  (-25 LOC; 12/12 chat_engine; 89 lib pass)
2026-05-26T03:00Z  M2-T2b implementer        DONE_W_CONCERNS  8441f0a  (3 pre-accepted concerns; streaming-only rewire +208/-152)
2026-05-26T03:05Z  M2-T2b spec-reviewer      compliant
2026-05-26T03:15Z  M2-T2b code-reviewer      approve-with-fixes (2 Important: reasoning gate regression + ChatChunk.usage discarded)
2026-05-26T03:25Z  M2-T2b.1 implementer      DONE             89e8450  (+44/-25; reasoning gate + usage harvest; 89/9 unchanged)
2026-05-26T04:00Z  M2-T2c implementer        DONE_W_CONCERNS  5589965  (2 pre-accepted; net -62 LOC; 3 dead helpers gone)
2026-05-26T04:05Z  M2-T2c spec-reviewer      compliant
2026-05-26T04:15Z  M2-T2c code-reviewer      approve  (3 Suggestions deferred: inspect_err style, ~95 LOC dup → M2-T5 helper task, top_p/top_k drop → exec-plan owner)
2026-05-26T04:16Z  M2-T2c user-test-validator structural-pass UT-DISSOLVE-002 + UT-004 (no regression; full lib test 89/9/1 unchanged)
2026-05-26T05:00Z  M2-T2d implementer        DONE             1b0004c  (Arc<TokioMutex<HashMap>> registry + RAII guard + cancel_stream + message_stop_stream IPC)
2026-05-26T05:05Z  M2-T2d spec-reviewer      compliant
2026-05-26T05:15Z  M2-T2d code-reviewer      approve-with-fixes (1 Important: pub fn without test; 2 Suggestions: Drop runtime probe + IPC input bound)
2026-05-26T05:25Z  M2-T2d.1 implementer      DONE             2a6d741  (3 fixes; 91/9 unchanged; UT-DISSOLVE-003 structural assertion pinned)
2026-05-26T06:00Z  M2-T3  implementer        DONE             1d572d0  (+55/-69 in services/session.rs; generate_title via chat_engine)
2026-05-26T06:05Z  M2-T3  spec-reviewer      compliant
2026-05-26T06:15Z  M2-T3  code-reviewer      approve  (2 Suggestions: error-log granularity + silent unwrap_or_default — debuggability only)
2026-05-26T06:16Z  M2-T3  user-test-validator structural-pass UT-DISSOLVE-004 (baseline 91/9 preserved)

### Task checklist

- [x] M1-T1: Create `src-tauri/src/models/llm_types.rs` with copied leaf types — commit `83d28a0`
- [x] M1-T2: Switch ALL `handbox_llm::types::*` import sites — commit `b758f56` (via pub-use re-exports per Decision Log)
- ~~M1-T3: Switch `src-tauri/src/storage/message_repository.rs` to local `MessageRole`~~ — absorbed into M1-T2 (see Decision Log).
- [x] M1-T4: Verify M1 with `cargo test` + DB JSON roundtrip — commit `21e038b` (test pins LlmMessageRole serde wire shape; M1 Exit Gate ✅)
- [x] M2-T1: Add `src-tauri/src/services/chat_engine.rs` with direct hand-ai dispatch — commit `f068875` + `a512c4f` (M2-T1.1 fix-pass)
- ~~M2-T2: Rewire `services/message.rs` chat flows through `chat_engine`~~ — split into M2-T2a/b/c/d per Decision Log (2026-05-26).
- [x] M2-T2a: Extend chat_engine API — commits `ba887a8` + `77ab4ef` (M2-T2a.1 fix-pass)
- [x] M2-T2b: Rewire streaming path in services/message.rs — commits `8441f0a` + `89e8450` (M2-T2b.1 fix-pass)
- [x] M2-T2c: Rewire non-stream path + delete dead helpers — commit `5589965` (net -62 LOC; both dispatch paths now through chat_engine)
- [x] M2-T2d: Cancellation source survey + wiring — commits `1b0004c` + `2a6d741` (M2-T2d.1 fix-pass)
- [ ] M2-T3: Rewire `services/session.rs` model lookups through hand-ai
- [ ] M2-T4: Rewire `services/model.rs` list_models through hand-ai catalog
- [ ] M2-T5: Replace `LlmClientError` import in `models/error.rs` with `hand_ai_model::ClientError`
- [ ] M3-T0 **(new)**: Flip `src-tauri/src/models/llm_types.rs` from re-exports to the verbatim copies M1-T1 originally wrote. Single atomic commit, build still clean (the local definitions replace `pub use handbox_llm::types::*` with `pub struct/enum {...}` bodies identical to handbox-llm's leaf types).
- [ ] M3-T1: Delete `src-tauri/crates/handbox-llm/` directory
- [ ] M3-T2: Drop `handbox-llm = …` from `src-tauri/Cargo.toml` and `src-tauri/crates/handbox-llm` from workspace dep graph
- [ ] M3-T3: Final compile + manual UI smoke test

## Surprises & Discoveries

(none yet — will be appended as work proceeds)

## Decision Log

**2026-05-25 — Local copies instead of re-export.** Two ways to dissolve a crate: (a) re-export hand-ai types from a thin shim, (b) copy the types HandBox needs into HandBox-local modules. (a) leaves HandBox coupled to whatever hand-ai chooses to ship next; the leaf types we use (e.g. `LlmMessageRole`) are tiny enums whose meaning is stable and HandBox-domain (we want our own `Tool` variant naming regardless of what hand-ai calls it). Copy.

**2026-05-25 — M1-T2 absorbs M1-T3 + service call sites (Rust nominal typing).** The original plan scoped M1-T2 to `storage/types/*` only, with `storage/message_repository.rs` deferred to M1-T3 and `services/*` deferred to M2. M1-T2's implementer dispatched and surfaced a hard contradiction: Rust types are nominal, not structural, so the moment `storage/types/message.rs::Message::role` is retyped from `handbox_llm::types::LlmMessageRole` to `crate::models::llm_types::LlmMessageRole`, every `services/*` consumer that reads `message.role` as the old type fails to compile (20 sites across 5 files). The plan as originally written would have committed a half-broken tree — that violates the skill's "atomic commit / clean tree" rule.

First resolution attempt: collapse M1-T2 + M1-T3 + the affected service/model call sites into one big mechanical sweep under M1-T2.

**2026-05-26 — M2-T2 split into 4 sub-tasks (session boundary resilience).** The original M2-T2 ("rewire services/message.rs through chat_engine") was a single ~500-line task that combined chat_engine API expansion (Phase A) with services/message.rs rewire (Phase B). On its first dispatch the implementer correctly BLOCKED on tool-call consumption in the non-stream path. On its second dispatch (revised v2 brief with terminal-tool-call aggregation + ChatMessage + hydrated_attachments), the implementer got mid-flight into Phase B when a session boundary hit, leaving the tree dirty with 18 compile errors and the next agent unable to recover the prior agent's context.

Resolution: split into four atomic sub-tasks. Each commit lands a clean compile + tests, so a session boundary in the middle cannot leave the next agent guessing.

- **M2-T2a** — chat_engine API expansion only (chat_engine.rs). ChatMessage / ChatToolCall / HydratedAttachment structs + ChatChunk.tool_calls + ChatOptions.hydrated_attachments + messages_to_context signature change + 10 existing tests retargeted to ChatMessage + 2 new tests. Build stays clean (no service callers exercise the new types yet).
- **M2-T2b** — services/message.rs streaming path rewire. Replace call_llm_api_stream's create_llm_client+chat_stream with chat_engine::stream_chat. Leave non-stream path untouched.
- **M2-T2c** — services/message.rs non-stream path rewire + delete helpers (convert_to_api_request / prepare_tools / llm_provider_from_provider) now that both paths are off the legacy adapter. Annotate the still-living MessageService.llm_config field with #[allow(dead_code)] — actual field removal is M2-T5 territory.
- **M2-T2d** — cancellation source. Survey existing Stop-button cancel mechanism (if any); if found, wire to ChatOptions.signal; if not, materialize a stream_id → CancellationToken registry. UT-DISSOLVE-003 acceptance hangs off this.

**2026-05-25 — M1-T1 strategy revised: verbatim copy → transitional re-exports.** The "collapsed sweep" attempt above ran into a deeper structural issue: heavy aggregate types (`LlmMessage`, `LlmRequest`, `LlmModel`, `LlmResponse`) hold leaf types as struct fields. Once we have two distinct nominal copies of each leaf type (one in handbox-llm, one in `crate::models::llm_types`), every heavy-aggregate construction or destructure site becomes a type error at the field boundary. M2 owns the aggregate migration; M1 cannot finish without it.

Two paths out:

- **(2) inter-copy `From` impls** — write 22 trait impls bridging local↔heavy leaf types. Orphan rule forces half of them to live inside `handbox-llm/src/...` which means editing handbox-llm; they're literally identity-shape copies, so the bridge code is pure noise that disappears in M3.
- **(7) `models/llm_types.rs` becomes a transitional re-export** — `pub use handbox_llm::types::{...}` instead of verbatim copies. `crate::models::llm_types::LlmMessageRole` IS `handbox_llm::types::LlmMessageRole` (same nominal type via re-export). The import sweep is zero-cost. M3 acquires one new pre-task: flip `models/llm_types.rs` from re-exports back to verbatim copies just before deleting the crate.

Pick (7). It keeps the build clean throughout, costs no conversion code, and only adds a single ~5-minute "flip" task at the M3 boundary that mechanically replays what M1-T1 originally did. The verbatim definitions M1-T1 wrote will be re-introduced into the same file at M3 time — they're not lost work, just deferred to the moment they're actually needed.

This makes M1-T2 trivial again: pure path-rewrite, single atomic commit, no semantic shift, no type-conversion plumbing.

**2026-05-25 — Preserve serde representations byte-for-byte.** HandBox's SQLite stores serialized `LlmMessageRole` and `LlmToolCall` JSON in TEXT columns. If the new local types have different `#[serde(rename_all = …)]` or field names, existing rows become unreadable. Every leaf type's serde shape is reproduced exactly, and an explicit deserialization roundtrip test pins the contract.

**2026-05-25 — Skip the legacy adapters wholesale.** Phase D (delete the 4 legacy chat adapters + 4 legacy model adapters) was pending under the prior overnight plan as a "validate first" safety net. Dissolving the crate makes them go away by definition — the question is moot once `handbox-llm/` is deleted. The escape hatch (`--no-default-features`) goes away too. Accept this trade.

**2026-05-25 — Hand-ai's `get_models` is the new source of truth for model lists.** ModelService currently calls `OpenAIFetcher` / `AnthropicFetcher` against the provider's `/v1/models`. After dissolution it reads `hand_ai_model::get_models(provider_id)`. Live `/v1/models` polling for "user added custom model" is out of scope and will be reintroduced later only if a user reports a missing model in the catalog.

## Outcomes & Retrospective

(To be filled at plan completion)

## Context and Orientation

Related documents:

- Earlier exec plan: `docs/exec-plans/hand-ai-integration.md` — the design contract for Path B (adapter) integration. This plan supersedes it by completing the absorption.
- Overnight summary: `docs/exec-plans/hand-ai-overnight-2026-05-25.md` — what landed yesterday + the deferred decisions list. Decisions 1, 3, 4, 7 from that doc are resolved by executing this plan.
- Hand-ai capabilities source: `/Users/wanggang/dev/00/hand-ai/crates/model/src/capabilities.rs` — read for the `ProviderCapabilities` / `ApiCapabilities` shape.
- Hand-ai types source: `/Users/wanggang/dev/00/hand-ai/crates/model/src/types.rs` — read for `Context / Message / AssistantMessage / ToolResultMessage / UserMessage / AssistantMessageEvent / Usage / StopReason / Tool`.

Key source files (current state, before this plan):

- `src-tauri/crates/handbox-llm/src/lib.rs` — the crate's public surface. After this plan: gone.
- `src-tauri/crates/handbox-llm/src/chat/hand_ai_adapter.rs` — the translation layer between HandBox's `LlmRequest`/`LlmChunkResponse` and hand-ai's `Context`/`AssistantMessageEvent`. The translation logic moves verbatim into `src-tauri/src/services/chat_engine.rs`; the dispatcher around it disappears.
- `src-tauri/crates/handbox-llm/src/hand_ai_catalog.rs` — the read of `get_providers / get_models / capabilities()`. Moves verbatim into `src-tauri/src/models/hand_ai_catalog.rs` (the existing IPC command in `commands/hand_ai.rs` will import from there instead).
- `src-tauri/src/services/message.rs` — the chat handler. Calls `create_llm_client + .chat_stream` and consumes `LlmChunkResponse` chunks in a streaming loop. After this plan: calls `chat_engine::stream_chat` and consumes the same chunks (a HandBox-local type defined in `chat_engine.rs`).
- `src-tauri/src/services/session.rs` — the session handler. Uses `create_llm_client` for title generation. Same rewire.
- `src-tauri/src/services/model.rs` — model list. After: reads `hand_ai_model::get_models(provider_id)` directly.
- `src-tauri/src/storage/types/{message,session,model}.rs` — DB-serializable types that wrap leaf types from `handbox-llm`. After: wrap the local copies from `src-tauri/src/models/llm_types.rs`.
- `src-tauri/src/storage/message_repository.rs` — reads/writes the `Message` struct; needs `LlmMessageRole` for filtering and parsing.
- `src-tauri/src/models/{error,message,model}.rs` — `error.rs` has `From<LlmClientError> for AppError`. After: `From<hand_ai_model::ClientError>`.
- `src-tauri/src/commands/hand_ai.rs` — Tauri IPC for `hand_ai_list_providers`. After: imports the catalog from `crate::models::hand_ai_catalog` instead of `handbox_llm::hand_ai_catalog`.

**Terms of art:**

- *Leaf type* — a small, self-contained type (enum or POD struct) with no behavior, only a serde representation. HandBox copies these into its own crate to break the dependency.
- *Translation shim* — `hand_ai_adapter.rs` today: takes a `LlmRequest`, builds a `Context`, calls hand-ai, takes the `AssistantMessageEvent`s back, builds `LlmChunkResponse`s. This plan deletes one half of the translation: HandBox stops building `LlmRequest` entirely. The other half (events → chunks) survives, just under a different name in `chat_engine.rs`.
- *Chunk* — HandBox's streaming unit, today `LlmChunkResponse`. The callbacks in `services/message.rs` consume one of these per event. After this plan, the local equivalent `ChatChunk` lives in `chat_engine.rs` and carries the same fields HandBox actually reads (delta text, delta reasoning, finish reason, usage on the terminal chunk).

## Plan of Work

Work is organized into three milestones. Each milestone is independently verifiable and leaves the codebase in a working state. The milestones must be executed in order: M1 swaps types under the codebase's feet while keeping the call graph identical; M2 swaps the call graph while keeping the types stable; M3 deletes the now-unreferenced crate.

### Milestone 1: Inline leaf types into HandBox

**Scope.** Every type from `handbox-llm` that HandBox's storage or services reference directly is copied into HandBox-local modules with serde representations preserved byte-for-byte. After this milestone the only remaining `handbox_llm::` imports are the heavy ones — `LlmRequest`, `LlmResponse`, `LlmChunkResponse`, `LlmProvider`, `create_llm_client`, `LlmClientError` — i.e. the orchestration surface that Milestone 2 will remove.

**M1-T1 — Create `src-tauri/src/models/llm_types.rs`** with verbatim copies of:

- `LlmMessageRole` (enum, `#[serde(rename_all = "lowercase")]`, 4 variants: System / User / Assistant / Tool, plus `as_str`, `Display`, `FromStr`)
- `LlmToolFunction` (struct, `name: String`, `arguments: String`, `#[serde(rename_all = "camelCase")]`)
- `LlmToolCall` (struct, `id`, `tool_type` renamed to `type`, `function: LlmToolFunction`)
- `LlmMessageAttachment` (struct, `name`, `mime_type`, `data: Vec<u8>`)
- `LlmReasoningEffortConfig`, `LlmResponsesReasoning`, `LlmThinkingConfig` (3 reasoning structs)
- `ModelPricing` (struct, `input_text: Option<String>`, `output_text: Option<String>`)
- `LlmModelParameter` (enum with `FromStr` impl)

Each copied type carries a 2-line comment pointing at the original `handbox-llm` source so a future reader knows the provenance: `// Copied from handbox-llm/src/chat/types.rs r.123 — serde repr stable, DB-bound.`

Add `pub mod llm_types;` to `src-tauri/src/models/mod.rs`.

**M1-T2 — Switch persistence types.** In `src-tauri/src/storage/types/message.rs`, replace `use handbox_llm::types::{LlmMessageRole, LlmToolFunction}` with `use crate::models::llm_types::{LlmMessageRole, LlmToolFunction}`. Same for `From<handbox_llm::types::LlmToolCall>` and `to_llm_tool_call` (becomes `From<crate::models::llm_types::LlmToolCall>` and `to_llm_tool_call() -> crate::models::llm_types::LlmToolCall`). Repeat for `storage/types/session.rs` (`LlmReasoningEffortConfig / LlmResponsesReasoning / LlmThinkingConfig`) and `storage/types/model.rs` (`ModelPricing`).

**M1-T3 — Switch repository imports.** In `src-tauri/src/storage/message_repository.rs`, the 13 sites that use `LlmMessageRole` (filtering, parsing the `role` TEXT column, building test fixtures) switch to `crate::models::llm_types::LlmMessageRole`.

**M1-T4 — Verify.** Run `cargo test -p handbox --lib storage::types`. Also write one new test in `storage/types/message.rs::tests` that constructs a `Message` with `role: LlmMessageRole::User`, serializes it to JSON, and asserts the JSON string contains `"role":"user"` — pinning the wire compatibility with existing DB rows.

**Exit Gate:**

- `cargo check --manifest-path src-tauri/Cargo.toml` clean (no `unused_imports` warnings related to `handbox_llm` in storage/services — only `LlmRequest/LlmResponse/LlmChunkResponse/LlmProvider/create_llm_client/LlmClientError` may still appear, all owned by M2)
- `cargo test -p handbox --lib storage::types message_roundtrip_preserves_fields` passes
- New `serde_repr_matches_legacy` test passes
- Branch compiles with `--no-default-features` AND with the default feature set
- Handoff log entry appended

### Milestone 2: Replace orchestration with direct hand-ai calls

**Scope.** `LlmClient`, `ChatClient` trait, `create_llm_client`, `LlmRequest`, `LlmResponse`, `LlmChunkResponse`, `LlmProvider`, `LlmRequestTool`, `LlmToolChoice`, `LlmConfigProvider`, `LlmClientError` — none of these survive M2. Services build a `hand_ai_model::Context` directly, call `hand_ai_model::stream_simple` directly, and consume `AssistantMessageEvent`s directly through a local helper in `src-tauri/src/services/chat_engine.rs`.

**M2-T1 — Add `src-tauri/src/services/chat_engine.rs`.** This module owns the chat dispatch end-to-end. Its public surface:

```rust
pub struct ChatProvider {
    pub provider_type: String,   // e.g. "openai" — must match Provider::as_str()
    pub base_url: String,        // optional override; empty string = use Model template's
    pub api_key: String,
}

pub struct ChatChunk {
    pub content: Option<String>,        // TextDelta payload
    pub reasoning: Option<String>,      // ThinkingDelta payload
    pub finish_reason: Option<String>,  // Done variant → "stop"/"length"/"tool_calls"
    pub usage: Option<ChatUsage>,       // Set only on the terminal chunk
    // Tool-call streaming lands in a later iteration (mirror what
    // hand_ai_adapter currently does — leave as TODO with a clear error
    // path for now).
}

pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

pub async fn stream_chat(
    provider: &ChatProvider,
    model_id: &str,
    messages: &[crate::storage::types::Message],
    options: ChatOptions,
) -> Result<
    impl futures::Stream<Item = Result<ChatChunk, AppError>> + Send + Unpin,
    AppError,
>;

pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ChatTool>,         // empty = no tools
    pub reasoning_effort: Option<String>,
    pub signal: Option<tokio_util::sync::CancellationToken>,
}

pub struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Internally `stream_chat` does what `hand_ai_adapter.rs` does today — look up the `Model` template via `hand_ai_model::get_model`, override `base_url`, construct `Context` (with prior assistant turns reconstructed, tool_name passthrough), build `SimpleStreamOptions`, call `stream_simple`, and map `AssistantMessageEvent`s to `ChatChunk`s. The difference is that `chat_engine.rs` sits inside HandBox, takes HandBox-shaped inputs, and emits HandBox-shaped outputs — no `LlmRequest` in sight.

A non-streaming `complete_chat(...)` mirror of the same shape lands at the same time, used by `services/session.rs::generate_title` (the only consumer that calls `llm_client.chat` non-stream today).

**M2-T2 — Rewire `services/message.rs`.** The two call sites that build a `LlmRequest` via `convert_to_api_request` and invoke `create_llm_client + .chat / .chat_stream` are rewritten:

- `convert_to_api_request` is renamed `build_chat_options` and returns `ChatOptions` (no `LlmRequest`). The messages and model_id are passed as separate arguments to `stream_chat`.
- The streaming loop continues to read `chunk.choices[0].delta.content` style fields, but reads them off `ChatChunk` instead. The shape match is intentional — minimal churn in the streaming-callback code that lives further down in `message.rs`.
- All `LlmRequest`, `LlmMessage`, `LlmResponse`, `LlmRequestTool`, `LlmToolChoice` imports go away.

**M2-T3 — Rewire `services/session.rs`.** The title-generation path uses non-stream `chat`. Switches to `chat_engine::complete_chat`. Drops the `use handbox_llm::{create_llm_client, LlmProvider}` import.

**M2-T4 — Rewire `services/model.rs`.** Replaces `create_llm_client(...).list_models(provider)` with `chat_engine::list_catalog_models(provider_type)` — a thin wrapper that reads `hand_ai_model::get_models(provider_type)` and maps each `model::Model` to HandBox's `Model` storage type. Models hand-ai doesn't know about (a hypothetical user-added custom id) are not returned by this path — the trade-off is recorded in the Decision Log; if a user complaint arrives, follow up with a separate live `/v1/models` fallback.

**M2-T5 — Replace error conversion.** `src-tauri/src/models/error.rs` currently has `From<handbox_llm::LlmClientError> for AppError`. Replace with `From<hand_ai_model::ClientError> for AppError`. The variant set is similar (Validation / Configuration / Transport / Api) — map them by category. Anything `chat_engine.rs` returns to `services/message.rs` is `Result<_, AppError>`, so `chat_engine` itself is responsible for the `ClientError → AppError` conversion at its boundary.

**Exit Gate:**

- `cargo check` clean both with default features and `--no-default-features` (the latter still builds because we've not yet deleted the crate; once M3 deletes it, only the default build remains).
- All existing lib tests pass: `cargo test --manifest-path src-tauri/Cargo.toml --lib` returns the same pass count as before M2, minus tests that referenced removed types (those are deleted, not adjusted).
- Manual smoke: build the Tauri app, send a chat message to OpenAI via the live UI, confirm streaming text appears and finish_reason fires. (No CI key. The author of M2 verifies locally.)
- Handoff log entry appended.

### Milestone 3: Delete the crate

**Scope.** With M1 and M2 done, no source file outside `src-tauri/crates/handbox-llm/` references the crate. The crate is now strictly dead weight.

**M3-T1 — Delete `src-tauri/crates/handbox-llm/`** as a directory. `rm -rf` plus a `git rm -r`.

**M3-T2 — Remove from `src-tauri/Cargo.toml`.** Drop the `handbox-llm = { path = "crates/handbox-llm" }` line. Drop the workspace-wide `default = ["hand-ai"]` feature block — feature parity is no longer needed because there's no alternative path. The `hand-ai-model` dependency stays exactly where it is.

**M3-T3 — Final compile + manual UI smoke.** `cargo build --manifest-path src-tauri/Cargo.toml` clean. Start `cargo tauri dev`, open the app, confirm:

  1. The provider picker still shows 30+ entries.
  2. Sending a chat message to an existing provider (e.g. OpenAI) produces a streaming response with content and a usage tally.
  3. Switching models within an existing session reflects in the next message's model field.
  4. The new providers IPC `hand_ai_list_providers` still returns a populated list (now sourced from `crate::models::hand_ai_catalog::list_providers`).

**Exit Gate:**

- `find src-tauri -name "handbox-llm*" -not -path "*/target/*"` returns empty
- `grep -rn "handbox_llm" src-tauri/src` returns empty
- `cargo build` clean; binary launches; manual UI smoke passes all four checks above
- Handoff log entry appended

## User Test Coverage

No `docs/user-tests/` set exists yet — none of this work is behavioural, so a formal user-test set was not authored. Runtime validation is per the manual smoke checks in each milestone's Exit Gate.

| Task | User-test cases covered |
|------|--------------------------|
| M1-T1 | — (refactor: type copy, no behavior) |
| M1-T2 | — (refactor: import rewrite, no behavior) |
| M1-T3 | — (refactor: import rewrite, no behavior) |
| M1-T4 | — (test scaffolding for serde repr compat) |
| M2-T1 | — (infra: new module, no behavior until M2-T2 wires it) |
| M2-T2 | Manual: chat works for openai (streaming), reasoning surfaces in UI |
| M2-T3 | Manual: title generation works on new chat |
| M2-T4 | Manual: provider models list populates after add-provider |
| M2-T5 | — (error wiring, surfaces on any failure path) |
| M3-T1 | — (deletion) |
| M3-T2 | — (Cargo.toml cleanup) |
| M3-T3 | Manual: full app launches and chats end-to-end |

## Concrete Steps

All commands assume working directory `/Users/wanggang/dev/00/handbox/handbox` (the repo root) unless stated otherwise.

**Pre-flight (run before starting):**

```
git checkout feat/hand-ai-integration
git status   # expect: nothing to commit, working tree clean
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet \
    config::llm_config \
    chat::hand_ai_adapter \
    hand_ai_catalog \
    storage::types
# Expected: every listed test family passes; record the counts so M2 can compare.
```

**During M1 (after each task):**

```
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib storage::types
```

After M1-T4 also run:

```
cargo test --manifest-path src-tauri/Cargo.toml --lib serde_repr_matches_legacy
# Expected: ok. 1 passed
```

**During M2 (after each task):**

```
cargo check --manifest-path src-tauri/Cargo.toml --features hand-ai
cargo test --manifest-path src-tauri/Cargo.toml --lib services::message services::session services::model
```

After M2-T2 also run the live smoke (requires a saved OpenAI API key in HandBox's DB):

```
npm run tauri dev
# Then in the app: send "say hi" to gpt-4o. Expect a streaming response with finish_reason = "stop".
```

**During M3:**

```
git rm -r src-tauri/crates/handbox-llm
# edit src-tauri/Cargo.toml manually
cargo build --manifest-path src-tauri/Cargo.toml --release
# Expected: clean build; binary at src-tauri/target/release/handbox

grep -rn "handbox_llm\|handbox-llm" src-tauri/src src-tauri/Cargo.toml \
    || echo "all references removed"
# Expected: "all references removed"
```

**Manual UI verification (after M3-T3):**

1. `npm run tauri dev` — wait for window.
2. Open Settings → Models → Add Provider → confirm dropdown lists ≥ 30 entries.
3. In an existing chat session with OpenAI configured, type "stream test" and send. Expect tokens to arrive incrementally and a final usage stat to render.
4. Switch model to a different OpenAI model mid-session; send another message; confirm it uses the new model.

## Validation and Acceptance

Static validation per milestone is described in each Exit Gate.

Runtime validation is the 4-step manual UI smoke after M3-T3. Acceptance criteria, observable to anyone running the app:

1. **Provider picker populated.** Adding a provider via the UI lists at least 30 distinct entries. The 5 hand-tuned ones (OpenAI, Anthropic, Google, OpenRouter, Deepseek) appear with their full icons. The rest appear with the `/logo-150.png` placeholder.
2. **Streaming chat works.** A message to a configured provider (OpenAI is the canonical test) produces visibly streamed tokens that compose into a coherent response. Tool calls, if the user activates a tool-using artifact, still fire.
3. **Cancellation works.** Hitting the stop button during streaming truncates the response within ~100ms (this is the wrapper-level gate hand-ai 0.2.0 provides; it survives the refactor because `chat_engine::ChatOptions::signal` plumbs through to `SimpleStreamOptions::signal`).
4. **No regression in non-chat IPCs.** Settings, message list rendering, MCP tool calls, image attachments — all surfaces that don't go through chat still work. Spot-check by editing one provider and verifying it persists; sending one image-attached message; running a chat in a new session.

If any of these break, roll back the entire branch with `git reset --hard origin/feat/hand-ai-integration~3` (the count is whatever the M3 series rolls in; specific commit pinned at M3 completion). The legacy adapters are gone after M3, so there is no in-place rollback to legacy chat — the only safe revert is a full branch reset to before M2 landed.

## Idempotence and Recovery

Each milestone leaves the working tree compilable and the lib tests passing. A partially completed milestone can be re-run safely:

- **M1**: rewriting imports is idempotent — running the rewrite twice produces the same result. If a `From` impl conversion breaks at compile time, fix the specific impl and re-run; no DB or filesystem damage possible.
- **M2**: more delicate because the new `chat_engine` module is incrementally built. Recovery strategy: if M2-T2 leaves the codebase in a non-compilable state and the implementer can't recover, `git restore src-tauri/src/services/message.rs` to revert just that file and start over. The new `chat_engine.rs` from M2-T1 stays.
- **M3**: irreversible by design. If the deletion turns up a hidden reference that compilation didn't catch (e.g. a dynamic `serde_json::from_str::<handbox_llm::types::LlmMessage>`), copy the missing piece into `models/llm_types.rs` and re-run. Do not restore `handbox-llm/`.

## Artifacts and Notes

Prototyping was not necessary — the existing `hand_ai_adapter.rs` is a working prototype of the translation logic that M2 reuses. The grep audit (commit `51d6288` summary) confirmed that `handbox-llm` types appear in exactly 14 files outside the crate itself, and that the majority of references are to a small set of leaf enums and structs that copy trivially.

Anticipated complexity hot spots, recorded so the implementer doesn't get surprised:

1. **`services/message.rs::convert_to_api_request`** (line 1479) is ~150 lines that build `LlmRequest` from the chat session. The same logic — just renamed and yielding `ChatOptions` instead of `LlmRequest` — survives in `build_chat_options`. The reasoning-effort branching (lines 1560-1620) is the densest; preserve it verbatim with type swaps only.
2. **Tool-call event aggregation.** Today `hand_ai_adapter.rs::event_to_chunk_result` punts on `ToolCallStart/Delta/End` events. `chat_engine.rs` inherits this gap. The first real user complaint about a streaming tool-call drives the fix.
3. **`Cargo.lock` churn.** Deleting `handbox-llm` causes Cargo to re-resolve and drop transitive entries (chrono, openai-rust, google-genai-rust may shift if they're now only pulled via hand-ai-model). Commit `Cargo.lock` alongside M3-T2.

## Interfaces and Dependencies

After this plan, the canonical chat surface for HandBox-internal code is:

In `src-tauri/src/services/chat_engine.rs`:

```rust
pub struct ChatProvider { pub provider_type: String, pub base_url: String, pub api_key: String }
pub struct ChatChunk { pub content: Option<String>, pub reasoning: Option<String>, pub finish_reason: Option<String>, pub usage: Option<ChatUsage> }
pub struct ChatUsage { pub prompt_tokens: i32, pub completion_tokens: i32, pub total_tokens: i32 }
pub struct ChatOptions { pub temperature: Option<f32>, pub max_tokens: Option<u32>, pub tools: Vec<ChatTool>, pub reasoning_effort: Option<String>, pub signal: Option<tokio_util::sync::CancellationToken> }
pub struct ChatTool { pub name: String, pub description: String, pub parameters: serde_json::Value }

pub async fn stream_chat(
    provider: &ChatProvider,
    model_id: &str,
    messages: &[crate::storage::types::Message],
    options: ChatOptions,
) -> Result<impl futures::Stream<Item = Result<ChatChunk, AppError>> + Send + Unpin, AppError>;

pub async fn complete_chat(
    provider: &ChatProvider,
    model_id: &str,
    messages: &[crate::storage::types::Message],
    options: ChatOptions,
) -> Result<ChatChunk, AppError>;

pub fn list_catalog_models(provider_type: &str) -> Vec<crate::storage::types::Model>;
```

In `src-tauri/src/models/llm_types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmMessageRole { System, User, Assistant, Tool }
impl LlmMessageRole { pub fn as_str(&self) -> &'static str { /* … */ } }
impl FromStr for LlmMessageRole { /* … */ }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolFunction { pub name: String, pub arguments: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall { pub id: String, #[serde(rename = "type")] pub tool_type: String, pub function: LlmToolFunction }

// LlmMessageAttachment, LlmReasoningEffortConfig, LlmResponsesReasoning,
// LlmThinkingConfig, ModelPricing, LlmModelParameter — all with serde reprs
// preserved verbatim from handbox-llm.
```

In `src-tauri/src/models/hand_ai_catalog.rs`:

```rust
// Moved verbatim from handbox-llm/src/hand_ai_catalog.rs.
pub struct HandAiProviderInfo { /* unchanged */ }
pub struct HandAiProviderCaps { /* unchanged */ }
pub struct HandAiModelInfo { /* unchanged */ }
pub fn list_providers() -> Vec<HandAiProviderInfo>;
```

In `src-tauri/src/commands/hand_ai.rs`:

```rust
use crate::models::hand_ai_catalog;   // was: handbox_llm::hand_ai_catalog
```

External dependencies (Cargo):

- `hand-ai-model = { git = "ssh://git@github.com/wanggang316/hand-ai.git", tag = "model-v0.2.0", package = "model" }` — pinned, kept exactly as it is today.
- `tokio-util = { version = "0.7", features = ["sync"] }` — needed for `CancellationToken` since HandBox's `services/` will name the type directly. Add to `src-tauri/Cargo.toml` if not already a transitive (verify with `cargo tree -p tokio-util`).
- `aliyun-oss-rs`, `openai-rust`, `google-genai-rust` — no longer direct dependencies of `src-tauri`. They become transitive (via hand-ai-model) or vanish entirely. Verify with `cargo tree --depth 1` post-M3 and prune the section if anything's left orphaned.

Removed dependencies (Cargo):

- `handbox-llm = { path = "crates/handbox-llm" }` — gone.
- All transitive removals fall out automatically when M3-T2 commits.
