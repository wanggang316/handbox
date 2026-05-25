# Hand-AI Integration — Execution Plan

Status: **planning**
Branch: `feat/hand-ai-integration`
Author: Claude (HandBox side) · paired with Claude (hand-ai side, pane `5C37A225-…`)
Last updated: 2026-05-25

## Goal

Adopt `hand-ai/crates/model` as the underlying LLM client backing HandBox's
`handbox-llm` abstraction, without disturbing HandBox's persistence schema,
IPC contract, or UI types.

## Strategy — Path B (adapter)

`handbox-llm`'s `LlmRequest` / `LlmChunkResponse` / `ChatClient` trait stay as
HandBox's **internal IDL**. A new adapter delegates the actual provider call
to `model::Client`. Existing four adapters (openai-completions, openai-responses,
anthropic, google) are kept around until parity is verified per-provider, then
deleted in a later pass.

Rejected alternatives:

- **Path A — full replace**: too much blast radius for one PR. `LlmMessageRole`,
  `LlmToolCall`, `ModelPricing`, `LlmThinkingConfig` etc. are leaked into
  `src-tauri/src/storage/types/*` and serialized into the SQLite DB. Migrating
  storage and migrating LLM dispatch are independent problems; doing them in one
  PR risks DB corruption on rollback.
- **Path C — side-by-side**: deferred to M2; useful for the future coding-agent
  panel but doesn't help the chat path today.

## Module placement

New file inside the existing `handbox-llm` crate, **not** a sibling crate:

```
src-tauri/crates/handbox-llm/src/chat/
  anthropic_adapter.rs            (existing)
  google_adapter.rs               (existing)
  openai_completions_adapter.rs   (existing)
  openai_responses_adapter.rs     (existing)
  hand_ai_adapter.rs              (NEW — wraps model::Client)
```

Why same crate, not a sibling crate:

1. `ChatClient` trait stays one source of truth.
2. `create_chat_client(api_type)` keeps a single switch statement.
3. Feature-flagging the new adapter lets us ship dark by default until each
   provider is verified.

## Cargo dep strategy

```toml
# src-tauri/crates/handbox-llm/Cargo.toml
[features]
default = []
hand-ai = ["dep:hand-ai-model"]

[dependencies]
hand-ai-model = { git = "https://github.com/wanggang316/hand-ai.git", tag = "model-v0.1.0", package = "model", optional = true }
```

Pinning to a tag (NOT branch) is mandatory: blocks on hand-ai issue **#34**.

### Open dependency question — RESOLVED 2026-05-25

hand-ai uses `openai-rust` / `google-genai-rust` at tagged versions
(`openai-rust@v0.2.1`, `google-genai-rust@v0.1.0`). HandBox previously used
the same forks but at `branch = master` / `branch = main`. Although the
commits behind tag and branch HEAD were identical (both `1beaefe2` and
`176effd6` respectively), Cargo treated the two URL/ref pairs as distinct
crates and double-linked.

**Fix**: HandBox switched both its top-level `src-tauri/Cargo.toml` and
`crates/handbox-llm/Cargo.toml` to the exact same git URL + tag form hand-ai
uses (no `.git` suffix, `tag = "v0.2.1"` / `tag = "v0.1.0"`). Verified with
`cargo tree -d --features hand-ai` from `crates/handbox-llm/` — `openai-rust`
and `google-genai-rust` no longer appear in the duplicates list. Remaining
duplicates (`block-buffer`, `core-foundation`, etc.) are unrelated transitive
ones that existed before `hand-ai-model` was added.

Full `cargo check --manifest-path src-tauri/Cargo.toml` passes; nothing in
HandBox's existing code regressed from the branch→tag switch.

## Translation layer

### `LlmRequest → model::Context`

| HandBox field                       | hand-ai field                          | Notes                                                                |
|-------------------------------------|----------------------------------------|----------------------------------------------------------------------|
| `messages: Vec<LlmMessage>`         | `messages: Vec<Message>`               | Per-message translation table below.                                  |
| (split from messages)               | `system_prompt: Option<String>`        | HandBox merges system into messages today; adapter pulls first system-role message out into `system_prompt`. |
| `tools: Option<Vec<LlmRequestTool>>`| `tools: Option<Vec<Tool>>`             | Names + JSON schemas map 1:1.                                         |
| `model: String`                     | `&Model` (looked up via `get_model`)   | Provider id + model id → `model::Model`. Lookup table maintained per-provider. |
| `temperature` / `top_p` / `max_tokens` | `SimpleStreamOptions.base.*`         | Direct mapping.                                                       |
| `stream: bool`                      | always true at this layer              | Non-stream calls go through `complete_simple` separately.             |
| `tool_choice: Option<LlmToolChoice>`| TBD                                    | Hand-ai equivalent not yet located; investigate before unblocking.    |

#### `LlmMessage → Message` mapping

| `LlmMessageRole` | `Message` variant | Notes                                          |
|------------------|-------------------|------------------------------------------------|
| `System`         | (extracted to `Context.system_prompt`) | First system message wins; subsequent ones concatenated. |
| `User`           | `Message::User(UserMessage{ … })`     | Content blocks: text / image / tool_result.    |
| `Assistant`      | `Message::Assistant(AssistantMessage{ … })` | Includes prior tool calls and reasoning.   |
| `Tool`           | `Message::ToolResult(ToolResultMessage{ … })` | tool_call_id and content.                |

### `AssistantMessageEvent → LlmChunkResponse`

hand-ai's event surface is **richer** than `LlmChunkResponse`'s OpenAI-style
delta-on-choice model. The adapter aggregates events into completion-style
chunks:

| hand-ai event                  | LlmChunkResponse                                                       |
|--------------------------------|------------------------------------------------------------------------|
| `Start { partial }`            | (no emit; record initial assistant message id)                         |
| `TextStart / TextDelta / TextEnd` | One chunk per `TextDelta` with `delta.content = delta`              |
| `ThinkingStart / Delta / End`  | Chunk with `delta.reasoning_content = delta` (HandBox already supports reasoning_content as a top-level delta field for OpenAI-completions reasoning models) |
| `ToolCallStart / Delta / End`  | Aggregate into `delta.tool_calls[<idx>]`. Emit `ToolCallEnd` as a full `LlmToolCall` in the next chunk. |
| `Done { reason, message }`     | Terminal chunk with `finish_reason` mapped from `StopReason`, populated `usage` if present. |
| `Error { reason, error }`      | Map to `LlmClientError::*` and return early from the stream.           |

`partial: AssistantMessage` on each event is **ignored by the adapter** — we
trust the deltas, and persisting partial messages is HandBox's responsibility
upstream.

#### `StopReason → finish_reason` table

| hand-ai `StopReason` | HandBox `finish_reason` |
|----------------------|-------------------------|
| `EndTurn`            | `"stop"`                |
| `MaxTokens`          | `"length"`              |
| `ToolUse`            | `"tool_calls"`          |
| `ContentFilter`      | `"content_filter"`      |
| `Cancelled` (proposed) | `"stop"` + `cancelled = true` (custom field, ignored by older clients) |
| `Error`              | propagated as `LlmClientError` instead, not as a finish_reason |

### Cancellation

hand-ai exposes `SimpleStreamOptions.base.signal: Option<CancellationToken>`
already (verified: `crates/model/src/stream.rs:80`). Hand-ai issue **#32**
formalizes the contract; the adapter wires HandBox's existing per-request
cancellation channel into this `signal`.

### API key + base URL injection — RESOLVED 2026-05-25

HandBox stores provider API keys encrypted in SQLite and must NOT leak them
to environment variables. Initial concern was that `hand_ai_model::Client`
reads keys from env (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, …).

Investigation of `crates/model/src/types.rs:552` shows `StreamOptions`
already has `api_key: Option<String>` — the per-request key, redacted in
Debug output. Adapter just sets it from `LlmProvider.api_key`:

```rust
let opts = SimpleStreamOptions {
    base: StreamOptions {
        api_key: Some(provider.api_key.clone()),
        signal: Some(cancel_token),
        timeout_ms: ...,
        ..Default::default()
    },
    reasoning: ...,
    ..Default::default()
};
hand_ai_model::stream_simple(&registry, &model, context, Some(opts))
```

For custom `base_url` (BYO endpoint), `Model` exposes `base_url: String`
publicly. Adapter clones the template `Model` returned by
`get_model(provider, model_id)` and overrides `base_url` from
`LlmProvider.base_url` before passing to `stream_simple`.

No env-var dance required. No #33b builder dependency.

## Provider catalog

HandBox today drives the provider-selection UI from `llm_config.json`. To
avoid divergence with hand-ai's internal registry, the adapter calls:

```rust
hand_ai_model::get_providers()           -> Vec<model::Provider>
hand_ai_model::get_models("openai")      -> Vec<model::Model>
```

and surfaces this through a new IPC command `hand_ai_list_providers`. This
**replaces** the static "providers" array in `llm_config.json`; the JSON
shrinks to just per-provider UI metadata (icon, color, parameter form layout)
keyed by provider id.

Blocks on hand-ai issue **#31** for the full capabilities surface
(streaming/tools/oauth/multimodal). Until #31 lands, the adapter ships with a
hard-coded capabilities table mirroring what we know about each built-in
provider.

## Milestones

| M  | Scope                                                                  | Status / Blockers     |
|----|------------------------------------------------------------------------|-----------------------|
| M0 | Exec plan committed; hand-ai issues #31–#36 filed; openai-rust / google-genai-rust dedup verified | **DONE** (commits 76592cb, afcfb8d) |
| M1 | `hand_ai_adapter.rs` skeleton compiles behind `--features hand-ai`; translation tests lock the contract | **DONE** (commit 9aad119) |
| M2 | One provider (openai) end-to-end through adapter for single-turn text-only chat | API key + base URL injection unblocked via `StreamOptions.api_key` / `Model.base_url` (see below). Still need three real type mismatches: assistant history metadata, tool result `tool_name`, user attachments. |
| M3 | All four existing providers (openai/anthropic/google/openrouter) routed through adapter; old adapters deleted | #32 cancellation contract finalized |
| M4 | Expose new providers (Bedrock, Groq, xAI, …) through provider-catalog IPC; UI provider picker driven by hand-ai introspection | #31 (in progress), #33a Cargo features |
| M5 | `hand-coding-agent` mounted as a separate IPC surface (deferred)       | #36 base_dir           |

## Risk register

1. **Streaming throughput**: hand-ai re-emits per-delta `partial: AssistantMessage`
   on every event. For long completions this is O(n²) allocation. Mitigation:
   adapter drops the `partial` field eagerly; trusts deltas only. Confirmed
   acceptable upstream by reading `stream_simple` source.
2. **`openai-rust` / `google-genai-rust` double-link**: see "Open dependency
   question" above. Mitigation: verified via `cargo tree -d` before merging
   M1.
3. **DB schema lock-in**: HandBox stores `LlmMessageRole` etc. as serialized
   enums in SQLite. As long as the adapter doesn't rename or reshape these
   types, DB stays compatible. **Adapter MUST NOT touch `storage/types/*` —
   that's Path A territory.**
4. **Cancellation token type leak**: HandBox cannot directly expose
   `tokio_util::sync::CancellationToken` to JS. The adapter wraps it in
   HandBox's existing `RequestHandle` abstraction (used today by the four old
   adapters for the same purpose).

## Coordination

Hand-ai-side Claude (pane `5C37A225-BE90-47E7-8327-CB39E9B272B6`) handles
issues #31–#36 on the hand-ai repo. HandBox-side Claude (this one,
pane `9BB81C91-9148-4F31-B4EF-45D39A18AB5E`) holds the integration branch
and only lands code once a hand-ai milestone unblocks it.

Sync points: after #34 tag (unblocks M1); after #32 contract finalized
(unblocks M3); after #31 lands (unblocks M4).
