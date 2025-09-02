// LLM 配置相关 IPC 命令

use crate::models::{AppError, FrontendProviderConfig, ProviderConfigsResponse};
use crate::services::llm_config::get_global_llm_config;

/// 获取所有可用的供应商配置（用于前端添加/编辑供应商）
#[tauri::command]
pub async fn get_provider_configs() -> Result<ProviderConfigsResponse, AppError> {
    let config = get_global_llm_config();
    
    let providers = config.providers
        .iter()
        .map(|p| FrontendProviderConfig {
            provider_type: p.provider_type.clone(),
            type_name: p.type_name.clone(),
            default_name: p.default_name.clone(),
            default_base_url: p.default_base_url.clone(),
            icon: p.icon.clone(),
            chat_api_type: p.chat_api_type.clone(),
            model_api_type: p.model_api_type.clone(),
            description: None, // 可以根据需要添加描述
        })
        .collect();

    let custom_providers = config.custom_providers
        .iter()
        .map(|p| FrontendProviderConfig {
            provider_type: p.provider_type.clone(),
            type_name: p.type_name.clone(),
            default_name: p.default_name.clone(),
            default_base_url: p.default_base_url.clone(),
            icon: p.icon.clone(),
            chat_api_type: p.chat_api_type.clone(),
            model_api_type: p.model_api_type.clone(),
            description: None,
        })
        .collect();

    Ok(ProviderConfigsResponse {
        providers,
        custom_providers,
    })
}

/// 根据供应商类型获取具体配置
#[tauri::command]
pub async fn get_provider_config_by_type(provider_type: String) -> Result<Option<FrontendProviderConfig>, AppError> {
    let config = get_global_llm_config();
    
    if let Some(provider_config) = config.get_provider_config(&provider_type) {
        Ok(Some(FrontendProviderConfig {
            provider_type: provider_config.provider_type.clone(),
            type_name: provider_config.type_name.clone(),
            default_name: provider_config.default_name.clone(),
            default_base_url: provider_config.default_base_url.clone(),
            icon: provider_config.icon.clone(),
            chat_api_type: provider_config.chat_api_type.clone(),
            model_api_type: provider_config.model_api_type.clone(),
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
        
        // 验证基本结构
        assert!(!result.providers.is_empty());
        assert!(!result.custom_providers.is_empty());
        
        // 验证第一个供应商有 type_name 字段
        let first_provider = &result.providers[0];
        assert!(!first_provider.type_name.is_empty());
        assert!(!first_provider.default_name.is_empty());
        
        // 验证 type_name 和 default_name 可能不同（对于某些供应商）
        println!("First provider: type_name='{}', default_name='{}'", 
                first_provider.type_name, first_provider.default_name);
        
        // 验证自定义供应商
        let first_custom = &result.custom_providers[0];
        assert!(!first_custom.type_name.is_empty());
        assert!(!first_custom.default_name.is_empty());
        
        println!("First custom provider: type_name='{}', default_name='{}'", 
                first_custom.type_name, first_custom.default_name);
    }

    #[tokio::test]
    async fn test_get_provider_config_by_type() {
        let result = get_provider_config_by_type("openai".to_string()).await.unwrap();
        
        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config.provider_type, "openai");
        assert!(!config.type_name.is_empty());
        assert!(!config.default_name.is_empty());
        
        println!("OpenAI config: type_name='{}', default_name='{}'", 
                config.type_name, config.default_name);
    }
}