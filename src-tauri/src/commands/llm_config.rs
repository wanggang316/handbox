use crate::config::llm_config::get_global_llm_config;
use crate::models::{AppError, ProviderConfig, ProviderConfigsResponse};

/// Returns all available provider configs for the frontend's add/edit provider UI.
#[tauri::command]
pub async fn get_provider_configs() -> Result<ProviderConfigsResponse, AppError> {
    let config = get_global_llm_config();

    let providers = config
        .providers
        .iter()
        .map(|p| ProviderConfig {
            provider_type: p.provider_type.clone(),
            type_name: p.type_name.clone(),
            default_name: p.default_name.clone(),
            default_base_url: p.default_base_url.clone(),
            icon: p.icon.clone(),
            description: None,
        })
        .collect();

    let custom_providers = config
        .custom_providers
        .iter()
        .map(|p| ProviderConfig {
            provider_type: p.provider_type.clone(),
            type_name: p.type_name.clone(),
            default_name: p.default_name.clone(),
            default_base_url: p.default_base_url.clone(),
            icon: p.icon.clone(),
            description: None,
        })
        .collect();

    Ok(ProviderConfigsResponse {
        providers,
        custom_providers,
    })
}

#[tauri::command]
pub async fn get_provider_config_by_type(
    provider_type: String,
) -> Result<Option<ProviderConfig>, AppError> {
    let config = get_global_llm_config();

    if let Some(provider_config) = config.get_provider_config(&provider_type) {
        Ok(Some(ProviderConfig {
            provider_type: provider_config.provider_type.clone(),
            type_name: provider_config.type_name.clone(),
            default_name: provider_config.default_name.clone(),
            default_base_url: provider_config.default_base_url.clone(),
            icon: provider_config.icon.clone(),
            description: None,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_provider_configs() {
        let result = get_provider_configs().await.unwrap();

        assert!(!result.providers.is_empty());
        assert!(!result.custom_providers.is_empty());

        let first_provider = &result.providers[0];
        assert!(!first_provider.type_name.is_empty());
        assert!(!first_provider.default_name.is_empty());

        // type_name and default_name may differ for some providers.
        println!(
            "First provider: type_name='{}', default_name='{}'",
            first_provider.type_name, first_provider.default_name
        );

        let first_custom = &result.custom_providers[0];
        assert!(!first_custom.type_name.is_empty());
        assert!(!first_custom.default_name.is_empty());

        println!(
            "First custom provider: type_name='{}', default_name='{}'",
            first_custom.type_name, first_custom.default_name
        );
    }

    #[tokio::test]
    async fn test_get_provider_config_by_type() {
        let result = get_provider_config_by_type("openai".to_string())
            .await
            .unwrap();

        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config.provider_type, "openai");
        assert!(!config.type_name.is_empty());
        assert!(!config.default_name.is_empty());

        println!(
            "OpenAI config: type_name='{}', default_name='{}'",
            config.type_name, config.default_name
        );
    }
}
