---
name: dissolve-handbox-llm
description: User-test set for the handbox-llm crate dissolution refactor. Authored by /hs-test-spec. Read `docs/user-test-patterns.md` for project-wide conventions.
---

# User Tests: Dissolve the handbox-llm crate

**Status:** Draft
**Author:** Claude (HandBox side), supervised by Gump
**Date:** 2026-05-25
**ExecPlan:** [docs/exec-plans/dissolve-handbox-llm.md](../exec-plans/dissolve-handbox-llm.md)

## Scope note

This refactor has **no user-visible new behavior**. It deletes the `handbox-llm` internal crate, rewires the chat/model orchestration to call `hand_ai_model::Client` directly, and inlines a handful of leaf types into HandBox-local modules. Every case below is a **regression assertion**: the behavior the refactor must not break. There is no product spec — coverage maps to ExecPlan task IDs in the Coverage matrix at the bottom.

## Personas Used

- `developer_operator` — the only persona; required to have at least one provider configured with a working API key (canonically: OpenAI gpt-4o).

## Journeys

### Journey 1: Provider catalog still lists 30+ entries after the refactor

**Persona:** `developer_operator`
**Outcome:** Opening the Add-Provider flow shows the full hand-ai catalog as it did before the dissolution.

#### Case `UT-DISSOLVE-001`: Add-provider dropdown lists ≥ 30 entries including hand-tuned + placeholder icons

**Covers:** ExecPlan tasks M3-T3 (full app smoke) and indirectly M2-T1..T4 (catalog wiring survives the orchestration rewrite)

**Preconditions:**
- HandBox built from the feature branch HEAD with `npm run tauri dev`.
- App started; ready signal per `docs/user-test-patterns.md` reached (window visible AND stdout contains `Successfully loaded LLM config`).
- No special DB state required; whatever providers the operator already configured are irrelevant — this case exercises the catalog, not stored providers.

**Steps:**
1. From the main window, navigate to **Settings → Models**.
2. Click **添加供应商** (Add Provider).
3. Open the provider-type dropdown / picker.
4. Read the visible list of provider entries.

**Assertions:**
1. The picker shows **at least 30 distinct provider entries**. Operator counts manually; the count must be ≥ 30.
2. Five entries appear with their hand-tuned icons (not the `/logo-150.png` placeholder): **OpenAI**, **Anthropic**, **Google**, **Openrouter**, **Deepseek**. Each shows its branded logo image.
3. At least three of these entries appear with the placeholder icon `/logo-150.png` (or with a broken-image rendering): **Groq**, **Xai**, **Mistral**. Names use the `humanize_id`-formatted form ("Groq", "Xai", "Mistral", "Github Copilot", etc.).
4. The `hand_ai_list_providers` IPC, called via WKWebView devtools console with `await window.__TAURI_INTERNALS__.invoke('hand_ai_list_providers')`, returns an array with `length >= 30`.

**Artifacts on FAIL:**
- Screenshot of the picker via Cmd+Shift+5 → `~/Downloads/handbox-ut-fail-UT-DISSOLVE-001-<timestamp>.png`
- IPC return JSON pasted into the case log

---

### Journey 2: Streaming chat against a live provider keeps working

**Persona:** `developer_operator`
**Outcome:** Sending a chat message to a configured provider produces a stream of tokens that compose into a coherent reply, with a final usage tally.

#### Case `UT-DISSOLVE-002`: OpenAI chat streams text and emits usage on completion

**Covers:** ExecPlan tasks M2-T2 (services/message.rs rewire) and M3-T3 (full smoke)

**Preconditions:**
- HandBox built from the feature branch HEAD with `npm run tauri dev`.
- App started; ready signal reached.
- A provider with id `openai` exists in the operator's DB with a valid `api_key` (verified by the operator).
- That provider has at least one model with id `gpt-4o` enabled and visible in the model picker.
- The operator has an active chat session with that provider + model selected.

**Steps:**
1. In the active chat session, type `stream test` in the input box.
2. Submit (Enter or click Send).
3. Watch the assistant message bubble for the next 10 seconds.
4. After the stream completes, click the message metadata expander (or hover) to inspect token-usage info.
5. In the Tauri stdout (tail of `npm run tauri dev`), grep for `[MessageService::call_llm_api_stream]`.

**Assertions:**
1. Within 3 seconds of submit, **at least one partial token** is visible in the assistant bubble (not just a spinner). Visible meaning the bubble contains non-empty rendered text.
2. The stream produces **multiple visible incremental updates** (the bubble's content grows over time, not appearing as one final block). Operator confirms by observation.
3. After streaming ends, the bubble contains a non-empty final response (any coherent reply to "stream test" is acceptable; the wording is not the assertion — only that it's non-empty).
4. The usage info shows non-zero `prompt_tokens`, `completion_tokens`, and `total_tokens`. The values displayed match the formula `total_tokens == prompt_tokens + completion_tokens` (allowing for cache-token deltas).
5. Tauri stdout contains the line `[MessageService::call_llm_api_stream] Using provider: ... (openai)` (case-insensitive on "openai") at least once during the run.

**Artifacts on FAIL:**
- Screenshot of the chat bubble final state
- The Tauri stdout snippet covering this turn, saved to `/tmp/handbox-ut-UT-DISSOLVE-002-<timestamp>.log`

---

### Journey 3: Cancellation during a stream stops generation promptly

**Persona:** `developer_operator`
**Outcome:** Hitting the stop control mid-stream halts generation within a tight bound, demonstrating the hand-ai wrapper-level cancellation gate survived the refactor.

#### Case `UT-DISSOLVE-003`: Stop button truncates a streaming response within 500ms

**Covers:** ExecPlan tasks M2-T1 (ChatOptions.signal plumbing), M2-T2 (call site wires it), M3-T3 (smoke)

**Preconditions:**
- Same as UT-DISSOLVE-002.
- Operator picks a prompt likely to produce a long response: `please count slowly from 1 to 100, one number per line`.

**Steps:**
1. Submit the prompt.
2. Wait until at least 5 lines of output are visible in the assistant bubble (confirms streaming has started).
3. Click the **stop** button (typically a square icon next to the input area).
4. Note the exact timestamp of the stop click (operator uses a wall clock or the system's screenshot timestamps).
5. Watch the bubble for 2 seconds after the stop click.

**Assertions:**
1. Within **500ms** of the stop click, the bubble stops growing — no new tokens appear after that bound. (Operator measures roughly by visual observation; "less than half a second" is the acceptance.)
2. The bubble retains the text that was streamed before the stop (it is not blanked out).
3. The session is immediately ready for the next message (input box accepts text, no spinner persists).
4. Tauri stdout for this turn contains no `panicked at` or `Error` line; cancellation is a clean exit, not a failure.

**Artifacts on FAIL:**
- Screenshot of the bubble before and after stop, with timestamps in the filenames
- Tauri stdout snippet for the turn

---

### Journey 4: Non-chat surfaces don't regress

**Persona:** `developer_operator`
**Outcome:** Operations that don't go through the chat dispatcher — provider editing, image attachments, new session creation — all still work after the crate is gone.

#### Case `UT-DISSOLVE-004`: Edit-provider / new-session / image-attachment all succeed

**Covers:** ExecPlan tasks M1-T2..T3 (storage type swap), M2-T4 (model list rewire), M3-T1..T2 (deletion)

**Preconditions:**
- Same baseline as UT-DISSOLVE-001 (app started, settings reachable).
- One provider already configured.
- An image file ≤ 5MB available on disk for the attachment step.

**Steps:**
1. **Edit existing provider:** Settings → Models → click the configured provider's row → click **编辑** (Edit). Change the displayed name to a new string. Click Save.
2. Close and re-open the Settings panel. Confirm the provider's name reflects the edit.
3. **Create a new chat session:** Click **新会话** (or whatever the new-session entry point is named). Confirm a fresh chat opens with the input box ready.
4. **Image attachment:** In the new chat, drag the image file (or use the attach button) to attach an image. Type `what is in this image?` and send.
5. Wait for the response.
6. **Tail the log** for any `panicked at`, `error[`, or `unwrap()` failure during steps 1-5.

**Assertions:**
1. After step 2, the provider list shows the edited name (persisted across the panel re-open).
2. After step 3, a new empty chat session exists with no prior messages.
3. After step 5, the assistant response contains **at least one non-empty visible message** referring to the image (any content is acceptable — the assertion is that the attachment was transmitted, not the response quality).
4. The Tauri stdout for this entire 5-step session contains **zero** lines matching `panicked at|FATAL|unwrap.*None`.
5. SQLite `SELECT count(*) FROM providers` returns a value ≥ 1 after step 1 (the edit persisted to disk, not just in memory).

**Artifacts on FAIL:**
- Screenshot at the step that broke
- Full Tauri stdout for the session, saved to `/tmp/handbox-ut-UT-DISSOLVE-004-<timestamp>.log`
- `sqlite3 <db> "SELECT * FROM providers" > /tmp/handbox-ut-UT-DISSOLVE-004-providers.txt`

---

## Coverage matrix

This refactor has no product-spec ACs. Coverage maps cases to ExecPlan tasks (from `docs/exec-plans/dissolve-handbox-llm.md`).

| ExecPlan task | Covered by |
|---|---|
| M1-T1 (create llm_types.rs) | — (refactor: type copy, no observable behavior; unit covered by serde-repr roundtrip test in M1-T4) |
| M1-T2 (storage/types/* import swap) | UT-DISSOLVE-004 (DB read/write still works) |
| M1-T3 (message_repository import swap) | UT-DISSOLVE-002 (chat history read/write goes through the repository), UT-DISSOLVE-004 |
| M1-T4 (verify M1 with cargo test + roundtrip) | — (static gate, asserted by cargo test) |
| M2-T1 (chat_engine.rs new module) | UT-DISSOLVE-002 (the stream path uses it), UT-DISSOLVE-003 (the signal plumbing belongs to it) |
| M2-T2 (services/message.rs rewire) | UT-DISSOLVE-002, UT-DISSOLVE-003 |
| M2-T3 (services/session.rs rewire) | UT-DISSOLVE-004 (new-session creation uses session service) |
| M2-T4 (services/model.rs catalog rewire) | UT-DISSOLVE-001 (model list under each provider), UT-DISSOLVE-004 |
| M2-T5 (error.rs From impl swap) | UT-DISSOLVE-002, UT-DISSOLVE-003 (any failure path in stream/cancel surfaces via this conversion) |
| M3-T1 (delete crate directory) | UT-DISSOLVE-001..004 (post-deletion app must still serve all four journeys) |
| M3-T2 (Cargo.toml cleanup) | — (build-time only; asserted by `cargo build` clean) |
| M3-T3 (final smoke) | UT-DISSOLVE-001..004 (this case IS the smoke) |

## Open coverage gaps

- **Tool-call streaming.** No case in this set exercises a tool call mid-stream. The ExecPlan acknowledges (see Surprises & Discoveries) that `event_to_chunk_result` punts on `ToolCallStart/Delta/End` events today; the refactor inherits the gap. If/when tool-call streaming is wired up, add `UT-DISSOLVE-005`.
- **Non-OpenAI providers.** UT-DISSOLVE-002/003 only probe OpenAI. Anthropic / Google / OpenRouter / Deepseek end-to-end probes are not in this set; they reach hand-ai via the same dispatch and should work, but a future user-test addendum should add per-provider cases.
- **OAuth providers.** GitHub Copilot, OpenAI Codex, Anthropic OAuth flows are surfaced via the catalog but not used by this refactor. Out of scope for the regression set.
