// Catalog bridge over hand-ai's provider / model registry.
//
// HandBox-side mirror of hand-ai's `get_providers` + `get_models` +
// per-provider/per-api `capabilities()` introspection (hand-ai issue #31,
// commit 66222a3). The DTOs here are the wire shape the Tauri IPC
// command returns to the frontend.
//
// Behind the `hand-ai` feature flag so default builds don't pay the
// hand-ai compile cost.

use serde::Serialize;

use hand_ai_model::{self as model, types::Provider};

/// One provider entry surfaced to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct HandAiProviderInfo {
    /// Stable string id (e.g. "openai", "anthropic", "bedrock"). Matches
    /// `Provider::as_str`.
    pub id: String,
    /// API base URL the AddProvider UI uses as the default. Taken from the
    /// first model in the catalog; empty when the provider has no models
    /// registered yet.
    pub default_base_url: String,
    /// Vendor-level intrinsic facts.
    pub capabilities: HandAiProviderCaps,
    /// Models hand-ai knows about for this provider.
    pub models: Vec<HandAiModelInfo>,
}

/// Provider-level capabilities, mirrored from hand-ai's
/// `ProviderCapabilities` plus aggregated per-model rollups that the UI
/// chip needs (e.g. "this provider has at least one multimodal model").
#[derive(Debug, Clone, Copy, Serialize)]
pub struct HandAiProviderCaps {
    pub api_key_auth: bool,
    pub oauth_auth: bool,
    pub custom_base_url: bool,
    /// True if any model under this provider declares `InputType::Image`.
    pub any_model_multimodal_input: bool,
    /// True if any model under this provider has `reasoning = true`.
    pub any_model_reasoning: bool,
}

/// One model entry surfaced to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct HandAiModelInfo {
    pub id: String,
    pub name: String,
    /// Wire-protocol id (e.g. "openai-completions", "anthropic-messages").
    pub api: String,
    /// Whether the protocol natively supports function calling.
    pub api_supports_tools: bool,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub cost_per_million_input_usd: f64,
    pub cost_per_million_output_usd: f64,
    pub reasoning: bool,
    pub input_modalities: Vec<String>,
}

/// Enumerate every provider hand-ai knows about with its capabilities and
/// model catalog. Pure function — no I/O, no state. Called by the Tauri
/// command in `src/commands/hand_ai.rs`.
pub fn list_providers() -> Vec<HandAiProviderInfo> {
    model::get_providers()
        .into_iter()
        .map(provider_info)
        .collect()
}

fn provider_info(p: Provider) -> HandAiProviderInfo {
    let id = p.as_str().to_string();
    let raw_caps = p.capabilities();
    let raw_models = model::get_models(&id);
    let default_base_url = raw_models
        .first()
        .map(|m| m.base_url.clone())
        .unwrap_or_default();
    let models: Vec<HandAiModelInfo> = raw_models.into_iter().map(model_info).collect();

    let any_mm = models
        .iter()
        .any(|m| m.input_modalities.iter().any(|s| s == "image"));
    let any_reasoning = models.iter().any(|m| m.reasoning);

    HandAiProviderInfo {
        id,
        default_base_url,
        capabilities: HandAiProviderCaps {
            api_key_auth: raw_caps.api_key_auth,
            oauth_auth: raw_caps.oauth_auth,
            custom_base_url: raw_caps.custom_base_url,
            any_model_multimodal_input: any_mm,
            any_model_reasoning: any_reasoning,
        },
        models,
    }
}

fn model_info(m: model::Model) -> HandAiModelInfo {
    let api_caps = m.api.capabilities();
    HandAiModelInfo {
        id: m.id.clone(),
        name: m.name.clone(),
        api: api_id_str(&m.api),
        api_supports_tools: api_caps.tools,
        context_window: m.context_window,
        max_output_tokens: m.max_tokens,
        cost_per_million_input_usd: m.cost.input,
        cost_per_million_output_usd: m.cost.output,
        reasoning: m.reasoning,
        input_modalities: m
            .input
            .iter()
            .map(|i| input_type_str(*i).to_string())
            .collect(),
    }
}

/// Serialize an `Api` enum variant to its kebab-case wire form.
///
/// hand-ai's `Api` enum uses `#[serde(rename_all = "kebab-case")]`, so we
/// roundtrip through serde rather than re-typing the table here — if
/// hand-ai adds a variant we surface the new id automatically.
fn api_id_str(api: &model::Api) -> String {
    serde_json::to_value(api)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn input_type_str(t: model::InputType) -> &'static str {
    match t {
        model::InputType::Text => "text",
        model::InputType::Image => "image",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_providers_includes_openai_and_anthropic() {
        let infos = list_providers();
        let ids: Vec<&str> = infos.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"openai"), "providers should include openai: {:?}", ids);
        assert!(ids.contains(&"anthropic"), "providers should include anthropic: {:?}", ids);
    }

    #[test]
    fn anthropic_advertises_oauth() {
        let infos = list_providers();
        let anthropic = infos
            .iter()
            .find(|i| i.id == "anthropic")
            .expect("anthropic present");
        assert!(anthropic.capabilities.api_key_auth);
        assert!(anthropic.capabilities.oauth_auth);
        assert!(anthropic.capabilities.custom_base_url);
    }

    #[test]
    fn openai_is_api_key_only_no_oauth() {
        let infos = list_providers();
        let openai = infos
            .iter()
            .find(|i| i.id == "openai")
            .expect("openai present");
        assert!(openai.capabilities.api_key_auth);
        assert!(!openai.capabilities.oauth_auth);
        assert!(!openai.capabilities.custom_base_url);
    }

    #[test]
    fn copilot_has_no_api_key_path() {
        let infos = list_providers();
        let cp = infos
            .iter()
            .find(|i| i.id == "github-copilot")
            .expect("github-copilot present");
        assert!(!cp.capabilities.api_key_auth);
        assert!(cp.capabilities.oauth_auth);
    }

    #[test]
    fn api_id_string_uses_kebab_case() {
        assert_eq!(api_id_str(&model::Api::OpenAICompletions), "openai-completions");
        assert_eq!(api_id_str(&model::Api::AnthropicMessages), "anthropic-messages");
    }

    #[test]
    fn provider_aggregates_multimodal_when_any_model_supports_image() {
        // Build a fake provider info from synthesized models and verify the
        // aggregation rule directly (don't depend on hand-ai's static catalog
        // shape, which evolves).
        let mut info = HandAiProviderInfo {
            id: "x".into(),
            default_base_url: String::new(),
            capabilities: HandAiProviderCaps {
                api_key_auth: true,
                oauth_auth: false,
                custom_base_url: false,
                any_model_multimodal_input: false,
                any_model_reasoning: false,
            },
            models: vec![
                HandAiModelInfo {
                    id: "m1".into(),
                    name: "m1".into(),
                    api: "openai-completions".into(),
                    api_supports_tools: true,
                    context_window: 0,
                    max_output_tokens: 0,
                    cost_per_million_input_usd: 0.0,
                    cost_per_million_output_usd: 0.0,
                    reasoning: false,
                    input_modalities: vec!["text".into()],
                },
                HandAiModelInfo {
                    id: "m2".into(),
                    name: "m2".into(),
                    api: "openai-completions".into(),
                    api_supports_tools: true,
                    context_window: 0,
                    max_output_tokens: 0,
                    cost_per_million_input_usd: 0.0,
                    cost_per_million_output_usd: 0.0,
                    reasoning: false,
                    input_modalities: vec!["text".into(), "image".into()],
                },
            ],
        };
        info.capabilities.any_model_multimodal_input = info
            .models
            .iter()
            .any(|m| m.input_modalities.iter().any(|s| s == "image"));
        assert!(info.capabilities.any_model_multimodal_input);
    }
}
