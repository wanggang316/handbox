// model_runtime — provider/model resolution + stream-option building, shared
// by the chat dispatch path and the coding-agent session constructor.
//
// These helpers stay independent of the chat streaming engine so the agent
// session constructor (`coding_agent_session`) and the model catalog refresh
// (`services::model`) can resolve a `hand_ai_model::Model` template and build
// `SimpleStreamOptions` without it. The carrier types (`ChatOptions` /
// `ChatTool` / `HydratedAttachment`) live here because `build_stream_options`
// consumes `ChatOptions`; `chat_engine` re-exports them for its own callers.

use std::collections::HashMap;

use tokio_util::sync::CancellationToken;

use hand_ai_model::{self as model, SimpleStreamOptions, StreamOptions, ThinkingLevel};

use crate::models::llm_types::ModelPricing;
use crate::models::AppError;
use crate::storage::types::model::ModelModality;

/// Per-call options. Mirrors a subset of HandBox's chat parameter UI.
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ChatTool>,
    /// Pass-through reasoning effort hint (e.g. "low" / "medium" / "high").
    /// Mapped to hand-ai's `SimpleStreamOptions.reasoning: Option<ThinkingLevel>`.
    pub reasoning_effort: Option<String>,
    /// External cancellation channel. None = uncancellable; Some flows into
    /// `SimpleStreamOptions.base.signal`.
    pub signal: Option<CancellationToken>,
    /// Service callers pre-load attachment bytes **keyed by message id**.
    /// When a `ChatMessage` has non-empty `attachment_ids`, chat_engine looks
    /// up the hydrated payloads here under the message's own `id` (the
    /// `attachment_ids` vec is a presence indicator only — its individual
    /// values are not used as keys). Missing entries cause the attachment
    /// to be silently dropped with a `tracing::warn!` log line (no failure).
    pub hydrated_attachments: HashMap<String, Vec<HydratedAttachment>>,
}

#[derive(Debug, Clone)]
pub struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Pre-loaded attachment payload. Hand-ai's `UserContentBlock::Image` needs
/// raw bytes + mime; HandBox's storage stores file paths, so the service
/// layer hydrates before calling `stream_chat` / `complete_chat`.
#[derive(Debug, Clone)]
pub struct HydratedAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// List the models hand-ai knows about under `provider_type`. Maps each
/// `hand_ai_model::Model` to HandBox's `storage::types::Model`. Used by
/// `services/model.rs` to refresh the model picker without going through
/// the per-protocol `/v1/models` endpoint.
pub fn list_catalog_models(provider_type: &str) -> Vec<crate::storage::types::Model> {
    let now = chrono::Utc::now().timestamp_millis();
    hand_ai_model::get_models(provider_type)
        .into_iter()
        .map(|m| hand_ai_to_handbox_model(provider_type, &m, now))
        .collect()
}

/// Resolve a `Model` template by `provider_id` (== provider_type) + `model_id`.
///
/// Catalog providers resolve from hand-ai's static catalog. Custom providers
/// (openai-compatible / anthropic-compatible) aren't catalog entries, so a
/// template is synthesized from the wire protocol the custom type speaks (the
/// user supplies the model id + base_url). Anything else is a real error.
pub(crate) fn resolve_model_template(
    provider_id: &str,
    model_id: &str,
) -> Result<model::Model, AppError> {
    if let Some(m) = hand_ai_model::get_model(provider_id, model_id) {
        return Ok(m);
    }
    if let Some(api) = custom_api_for_provider_type(provider_id) {
        return Ok(synthesize_custom_model(model_id, api));
    }
    // OpenRouter is a dynamic aggregator fronting thousands of upstream models
    // that come and go (incl. ":free" variants), so hand-ai's *static* catalog
    // snapshot is necessarily incomplete and can lag the user's locally synced
    // model list. The provider API is the real authority: synthesize an
    // OpenAI-protocol template (OpenRouter speaks OpenAI completions; `base_url`
    // is filled in by `resolve_model` from the provider config) and let
    // OpenRouter validate the id. Fixed-catalog providers (openai, anthropic, …)
    // keep erroring on unknown ids.
    if provider_id == "openrouter" {
        return Ok(synthesize_custom_model(
            model_id,
            model::Api::OpenAICompletions,
        ));
    }
    Err(AppError::validation_error(&format!(
        "chat_engine: model '{}' not registered under provider '{}'",
        model_id, provider_id
    )))
}

/// Resolve the model template and override `base_url` from the caller-supplied
/// `ChatProvider` (mandatory for custom providers — the synthesized template
/// has no endpoint of its own).
pub(crate) fn resolve_model(
    provider_id: &str,
    model_id: &str,
    base_url: &str,
) -> Result<model::Model, AppError> {
    let mut m = resolve_model_template(provider_id, model_id)?;
    if !base_url.is_empty() {
        m.base_url = base_url.to_string();
    }
    Ok(m)
}

/// Map a HandBox custom-provider type to the hand-ai wire protocol it speaks.
///
/// Custom providers are HandBox-owned onboarding templates for unlisted
/// OpenAI-/Anthropic-compatible endpoints (local LLMs, proxies, vendors not in
/// hand-ai's catalog). hand-ai can't know about them, so HandBox owns this
/// fixed mapping. Returns `None` for catalog provider types.
pub(crate) fn custom_api_for_provider_type(provider_type: &str) -> Option<model::Api> {
    match provider_type {
        "openai-compatible" => Some(model::Api::OpenAICompletions),
        "anthropic-compatible" => Some(model::Api::AnthropicMessages),
        _ => None,
    }
}

/// The chat-method tags a manually-added model under a custom provider should
/// carry so it renders in the picker. `None` for non-custom provider types.
pub fn custom_provider_supported_methods(provider_type: &str) -> Option<Vec<String>> {
    custom_api_for_provider_type(provider_type).map(supported_methods_for_api)
}

/// Build a minimal `Model` template for a custom-provider model that isn't in
/// hand-ai's catalog. Stream dispatch keys off `api` (client.rs
/// `registry.get(&model.api)`), so only `api` + `base_url` (filled by the
/// caller) are load-bearing. `provider` is metadata only — it feeds the env-key
/// fallback (we always pass an explicit key) and a GitHubCopilot special-case
/// (avoided here), so a same-protocol placeholder is safe. Sizes are generous
/// "unknown but sane" defaults so the model's own cap doesn't truncate the
/// user's request; the actual limits come from `ChatOptions`.
fn synthesize_custom_model(model_id: &str, api: model::Api) -> model::Model {
    let provider = match api {
        model::Api::AnthropicMessages => model::types::Provider::Anthropic,
        _ => model::types::Provider::Openrouter,
    };
    model::Model {
        id: model_id.to_string(),
        name: model_id.to_string(),
        api,
        provider,
        base_url: String::new(),
        reasoning: false,
        input: vec![model::InputType::Text],
        cost: model::Cost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
        headers: None,
        // None lets hand-ai auto-detect OpenAI-compat quirks from base_url.
        compat: None,
        thinking_level_map: None,
    }
}

#[allow(clippy::field_reassign_with_default)]
// StreamOptions and SimpleStreamOptions are #[non_exhaustive] in hand_ai_model,
// so FRU (`..Default::default()`) is illegal from outside the defining crate —
// hence mutate-a-default instead.
pub(crate) fn build_stream_options(options: &ChatOptions, api_key: &str) -> SimpleStreamOptions {
    let mut base = StreamOptions::default();
    base.api_key = Some(api_key.to_string());
    base.temperature = options.temperature;
    base.max_tokens = options.max_tokens;
    base.signal = options.signal.clone();
    let mut opts = SimpleStreamOptions::default();
    opts.base = base;
    opts.reasoning = options
        .reasoning_effort
        .as_deref()
        .and_then(parse_thinking_level);
    opts
}

fn parse_thinking_level(s: &str) -> Option<ThinkingLevel> {
    match s.to_ascii_lowercase().as_str() {
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "xhigh" | "x-high" | "extra-high" => Some(ThinkingLevel::Xhigh),
        _ => None,
    }
}

fn hand_ai_to_handbox_model(
    provider_id: &str,
    m: &model::Model,
    now: i64,
) -> crate::storage::types::Model {
    let input_modalities: Vec<ModelModality> = m
        .input
        .iter()
        .map(|i| match i {
            model::InputType::Text => ModelModality::Text,
            model::InputType::Image => ModelModality::Image,
        })
        .collect();
    // hand-ai's Model only describes input modalities; outputs are implicitly
    // text for every chat-completion-style API.
    let output_modalities = vec![ModelModality::Text];

    let context_length = i32::try_from(m.context_window).ok();
    let output_max_tokens = i32::try_from(m.max_tokens).ok();

    let pricing = Some(ModelPricing {
        currency: Some("USD".to_string()),
        input_text: Some(m.cost.input as f32),
        output_text: Some(m.cost.output as f32),
    });

    crate::storage::types::Model {
        id: m.id.clone(),
        provider_id: provider_id.to_string(),
        name: m.name.clone(),
        context_length,
        output_max_tokens,
        supported_features: None,
        description: None,
        input_modalities: Some(input_modalities),
        output_modalities: Some(output_modalities),
        metadata: None,
        pricing,
        url: None,
        supported_parameters: None,
        default_parameters: None,
        max_parameters: None,
        supported_methods: Some(supported_methods_for_api(m.api)),
        model_created_at: None,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}

/// Map a hand-ai `Api` value to the short chat-method tags the UI uses to pick
/// a parameter set. Every synced provider must get a non-empty result: an empty
/// one makes `is_method_supported` false, and `get_provider_models` then filters
/// every model out at the IPC boundary. Older DB rows may still hold `None`.
fn supported_methods_for_api(api: model::Api) -> Vec<String> {
    use model::Api;
    match api {
        Api::OpenAICompletions | Api::MistralConversations => vec!["completions".to_string()],
        Api::OpenAIResponses | Api::AzureOpenAiResponses | Api::OpenAICodexResponses => {
            vec!["responses".to_string()]
        }
        // Anthropic / Bedrock-Converse share OpenAI Completions' UI parameter set
        // (temperature / top_p / max_tokens / streaming); the actual wire
        // dispatch is owned by hand-ai's Client via `Model.api`, not this tag.
        Api::AnthropicMessages | Api::BedrockConverseStream => vec!["completions".to_string()],
        Api::GoogleGenerativeAi | Api::GoogleGeminiCli | Api::GoogleVertex => {
            vec!["google_generate_content".to_string()]
        }
        // In-memory test harness — no UI surface.
        Api::Faux => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_methods_for_api_covers_every_variant() {
        // The exhaustive match already forces a compile-time decision for a new
        // hand-ai Api variant; this pins the runtime semantics of the existing
        // set.
        use hand_ai_model::Api;

        assert_eq!(
            supported_methods_for_api(Api::OpenAICompletions),
            vec!["completions".to_string()],
            "OpenAI-Completions wire → ChatMethod::Completions parameter set"
        );
        assert_eq!(
            supported_methods_for_api(Api::OpenAIResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::AzureOpenAiResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::OpenAICodexResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::AnthropicMessages),
            vec!["completions".to_string()],
            "Anthropic wire uses Completions parameter set for UI"
        );
        assert_eq!(
            supported_methods_for_api(Api::BedrockConverseStream),
            vec!["completions".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleGenerativeAi),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleGeminiCli),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleVertex),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::MistralConversations),
            vec!["completions".to_string()],
        );
        assert!(
            supported_methods_for_api(Api::Faux).is_empty(),
            "Faux is the in-memory test harness — no UI surface"
        );
    }

    #[test]
    fn openrouter_model_absent_from_catalog_synthesizes() {
        // An OpenRouter model the local sync cached but the current static catalog
        // does not list must still resolve: the aggregator, not the catalog, is
        // the authority on ids, so resolution synthesizes a template instead of
        // erroring "not registered under provider 'openrouter'".
        assert!(
            hand_ai_model::get_model("openrouter", "deepseek/deepseek-v4-flash:free").is_none(),
            "precondition: model is genuinely absent from the static catalog"
        );
        let m = resolve_model_template("openrouter", "deepseek/deepseek-v4-flash:free")
            .expect("openrouter catalog-miss must synthesize, not error");
        assert_eq!(m.api, hand_ai_model::Api::OpenAICompletions);
        assert_eq!(m.id, "deepseek/deepseek-v4-flash:free");
    }

    #[test]
    fn non_openrouter_unknown_model_still_errors() {
        // Only the OpenRouter aggregator gets the synthesize fallback. Fixed-catalog
        // providers must keep erroring on unknown ids (no doomed API call).
        assert!(resolve_model_template("openai", "no-such-model-9999").is_err());
        assert!(resolve_model_template("anthropic", "no-such-model-9999").is_err());
    }

    #[test]
    fn custom_api_mapping_covers_known_types() {
        assert_eq!(
            custom_api_for_provider_type("openai-compatible"),
            Some(hand_ai_model::Api::OpenAICompletions)
        );
        assert_eq!(
            custom_api_for_provider_type("anthropic-compatible"),
            Some(hand_ai_model::Api::AnthropicMessages)
        );
        assert_eq!(custom_api_for_provider_type("openai"), None);
        assert_eq!(custom_api_for_provider_type("groq"), None);
    }

    #[test]
    fn custom_provider_supported_methods_maps_to_completions() {
        assert_eq!(
            custom_provider_supported_methods("openai-compatible"),
            Some(vec!["completions".to_string()])
        );
        assert_eq!(
            custom_provider_supported_methods("anthropic-compatible"),
            Some(vec!["completions".to_string()])
        );
        assert_eq!(custom_provider_supported_methods("openai"), None);
    }

    #[test]
    fn synthesize_custom_model_builds_streamable_template() {
        // openai-compatible → OpenAICompletions template, dispatch-ready.
        let m = synthesize_custom_model("my-local-llm", hand_ai_model::Api::OpenAICompletions);
        assert_eq!(m.id, "my-local-llm");
        assert_eq!(m.api, hand_ai_model::Api::OpenAICompletions);
        // provider is a same-protocol placeholder, never GitHubCopilot (which
        // would trip the special-case in the openai-completions provider).
        assert_ne!(m.provider, hand_ai_model::types::Provider::GitHubCopilot);
        assert!(
            m.max_tokens > 0,
            "non-zero cap so requests aren't truncated"
        );
        assert!(m.compat.is_none(), "compat auto-detected from base_url");

        // anthropic-compatible → AnthropicMessages + Anthropic placeholder.
        let a = synthesize_custom_model("claude-proxy", hand_ai_model::Api::AnthropicMessages);
        assert_eq!(a.api, hand_ai_model::Api::AnthropicMessages);
        assert_eq!(a.provider, hand_ai_model::types::Provider::Anthropic);
    }
}
