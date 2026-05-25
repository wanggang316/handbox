# Overnight Hand-AI Integration Push — 2026-05-25 → 2026-05-26 AM

Branch: `feat/hand-ai-integration`
Author: Claude (HandBox side), unattended
Audience: Gump, reading in the morning

## What landed

| Phase | Commit | What changed |
|------|--------|--------------|
| A | `0172182` | Deleted entire OSS supplement subsystem (oss_client.rs, supplement.rs, aliyun-oss-rs dep, supplement_file/fields config, ModelSupplement* types, merge_supplement). Net −873 lines. |
| B-1 | `bcffe03` | hand-ai adapter now reconstructs prior assistant turns in history. Looks up the current request's model template once via `get_model(provider_id, model_id)` and reuses its api/provider for every reconstructed turn. Models not in catalog surface a clear error instead of fabricating defaults. Reasoning/text/tool_calls all preserved. |
| B-2 | `bcffe03` | Tool result `tool_name` recovered by walking back through prior assistant turns. Orphan tool results (no matching call_id) surface a clear error. |
| B-3 | `60daf12` | `LlmClient::new` picks chat backend via new `build_chat_client` helper. When `hand-ai` feature is on AND provider_type matches the hand-ai catalog → routes through `HandAiChatClient`. Otherwise legacy adapter. |
| (flip) | `5e1c339` | `hand-ai` is now a **default feature** on src-tauri. `cargo tauri dev` (no flags) gets the new backend behavior. `--no-default-features` re-selects legacy-only as the escape hatch. |
| C | `2d4286e` | `LlmConfig::load_from_app` appends one synthesized `ProviderConfig` per hand-ai catalog entry that isn't already in `llm_config.json`. Picker now shows 30+ providers automatically. Existing 5 entries keep their hand-tuned name/icon. |
| C-icons | `ec2a5ec` | Synthesized providers use `/logo-150.png` placeholder to avoid broken-image icons for the 25+ new entries until art ships. |

All commits are atomic, restorable, and on `feat/hand-ai-integration`. Total commits since `main`: 22.

## Test status

- `cargo test --features hand-ai -p handbox-llm --lib` → **35/35 pass**
- `cargo test --lib -p handbox-llm` (default feature off) → **9/9 pass**
- `cargo test --lib` on src-tauri root (handbox app code) → **86/86 pass** (none of mine added there; just confirms no regression in app-level config/services tests)
- Full `cargo check` on src-tauri root → **clean** both with and without `--features hand-ai`
- Pre-existing failure unrelated to me: `tests/test_supported_methods.rs::test_to_snake_case` — fails on `main` without my diff. Asserts `to_snake_case("UPPERCASE") == "u_p_p_e_r_c_a_s_e"` but the implementation returns `"uppercase"`. Not in scope; flagged for separate triage.

## What to verify in the morning

1. `cargo tauri dev` — starts cleanly, no compile errors.
2. Open the app → **Settings → Models → Add Provider** dropdown should now list 30+ entries (5 with their hand-tuned names + icons; 25+ with generic placeholder icon + title-cased names).
3. Pick an existing provider (e.g. **OpenAI**), confirm chat still works end-to-end. Behaviorally the only difference vs. yesterday is that the request goes through hand-ai 0.2.0 instead of HandBox's local openai_completions_adapter. Streaming, cancellation, tool calls should all still work.
4. Pick a new provider (e.g. **Groq**), enter your real API key, send a message. Should work end-to-end; if not, ping me with the error.
5. If anything is broken: `cargo tauri dev --no-default-features` rolls back to all-legacy behavior immediately. No code revert needed.

## Decisions I deferred to you

These were judgment calls where I either lacked context or the scope was too big for a single overnight push.

### Architecture / scope

1. **Phase D — delete the 4 legacy chat adapters + 4 legacy model adapters.**
   - With `hand-ai` default-on AND all 5 existing HandBox providers being in hand-ai's catalog, the legacy adapters are dead code in default builds.
   - Kept as safety net for `--no-default-features` until you confirm live chat works.
   - Net deletion potential: ~2300 lines. Recommend deletion after a day or two of validation. Mechanically: delete `chat/{anthropic,google,openai_completions,openai_responses}_adapter.rs` + `model/{anthropic,google,openai,openrouter}_adapter.rs`, drop `LlmApiType` dispatch, drop the `chat_api_type` / `model_api_type` strings from `llm_config.json`.

2. **Provider-specific icons for the 25+ new vendors.**
   - All currently use `/logo-150.png`. Per-provider art needs a designer.
   - Mechanical follow-up: drop `logo-{provider_id}.png` files into `static/` and change one line in `augment_with_hand_ai_providers`:
     ```rust
     icon: format!("/logo-{}.png", hp.id),
     ```
   - Alternatively, a Svelte `<ProviderIcon>` component with `onerror` fallback would let us point at per-provider URLs and gracefully degrade. ~30 lines of frontend work.

3. **Model list fetch for new providers.**
   - HandBox's `ModelClient` still calls `OpenAIFetcher` against the provider's base_url for `/v1/models`. Works for openai-compatible vendors (Groq, OpenRouter, Mistral, Cerebras, xAI). Will likely fail for Anthropic-style, Google-style, Bedrock, Vertex (auth flows differ).
   - Two paths:
     a. Have `ModelService::list_models` short-circuit to hand-ai's static `get_models(provider_id)` catalog when provider is in hand-ai. Faster (no network), guaranteed correct.
     b. Build provider-specific fetchers — duplicates hand-ai's work.
   - I'd vote (a). ~50 lines in `services/model.rs`. Couldn't fit in tonight cleanly without also pulling in hand-ai's `Model` → HandBox's `Model` translation, which I was nervous about doing speculatively.

4. **`chat_api_type` / `model_api_type` strings on synthesized providers.**
   - Currently hardcoded to `"openai-completions"` / `"openai"` regardless of the actual hand-ai protocol. They're never read on the hand-ai chat path (`build_chat_client` short-circuits before they matter). But if anyone introspects these strings for display or analytics, they'll see "openai-completions" for Anthropic, Google, Bedrock, etc. — misleading.
   - Mechanical follow-up: either derive them properly from hand-ai's `Api` enum, OR drop the fields from `ProviderConfig` entirely (since hand-ai routing has replaced their purpose).

### Smaller stuff

5. **`strip_model_version_suffix`** in `handbox-llm/src/model/model_client.rs` — kept under `#[allow(dead_code)]`. It's tested and may be useful for future provider id normalization. Delete if you're sure it's never wanted.

6. **`test_to_snake_case`** — pre-existing failure (see Test status). Fix or delete the assertion, but not part of this integration.

7. **`hand_ai_list_providers` IPC command** — still exposed but now redundant since `get_provider_configs` carries the same data. Keep for direct introspection / future tooling, or delete. I lean keep — it's eight lines.

8. **DB migration for storing provider rows with new provider_type values** — none needed. `provider_type` is just a string column. Existing rows untouched; new rows can be any of the 30+ ids.

## What I did NOT touch

- `chat_methods` in `llm_config.json` — UI parameter metadata, HandBox-domain.
- Provider icon resources in `static/` — design work.
- `handbox-llm` types leaked into `storage/types/*` — Path B explicitly keeps these as HandBox's internal IDL.
- `services/message.rs` orchestration logic — works through the trait, unchanged.

## hand-ai-side decisions that shaped tonight's work

- Confirmed `model-v0.2.0` tag (commit `cfc813c`) is the pinned version. Contains #1/#31/#32/#35/#36. Defers #33a/#33b to 0.3.0.
- `Stream/SimpleStreamOptions` are `#[non_exhaustive]`. HandBox uses mutate-default everywhere, no FRU.
- Wrapper-level cancellation gate in `stream_simple` — any provider gets cancelled even if it doesn't internally honor `signal`. Strong safety net.

## Risk register

- **High**: Live chat for the 5 existing providers now flows through hand-ai. If any provider-specific quirk doesn't translate (reasoning format, tool call argument encoding, image handling), it will surface in real usage tomorrow. Mitigation: `--no-default-features` rolls back instantly.
- **Medium**: Synthesized providers haven't been tested end-to-end with a real API call. The dispatch logic is structurally correct (proven by 35/35 tests), but the first real Groq/xAI/Mistral conversation might hit edge cases.
- **Low**: Model list fetch for new providers may 404 on non-OpenAI-compatible endpoints. Workaround is to manually add models or wait for the M3 follow-up that wires `ModelService` to hand-ai's static catalog.
