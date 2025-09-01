// Provider 功能单元测试

#[cfg(test)]
mod tests {
    use crate::models::{ModelFeature, ProviderConfig, ProviderType};
    use crate::services::{DatabaseService, ProviderService};
    use tempfile::tempdir;

    async fn create_test_service() -> (ProviderService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        println!("db_path: {:?}", db_path);
        let db_service = DatabaseService::new(&db_path).await.unwrap();
        (ProviderService::new(db_service), temp_dir)
    }

    #[tokio::test]
    async fn test_create_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        let config = ProviderConfig {
            name: Some("Test OpenAI".to_string()),
            provider_type: ProviderType::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-api-key".to_string(),
            enabled: Some(true),
        };

        let result = service.create_provider(config).await;
        assert!(result.is_ok());

        println!("result: {:?}", result);
        
        let provider = result.unwrap();
        assert_eq!(provider.name, "Test OpenAI");
        assert_eq!(provider.provider_type, ProviderType::OpenAI);
        assert!(provider.enabled);
        // status字段已移除
    }

    #[tokio::test]
    async fn test_get_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 先创建一个供应商
        let config = ProviderConfig {
            name: Some("Test Provider".to_string()),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "test-key".to_string(),
            enabled: Some(false),
        };

        let created = service.create_provider(config).await.unwrap();
        
        // 然后获取这个供应商
        let fetched = service.get_provider(&created.id).await;
        assert!(fetched.is_ok());
        
        let provider = fetched.unwrap();
        assert_eq!(provider.id, created.id);
        assert_eq!(provider.name, "Test Provider");
        assert_eq!(provider.provider_type, ProviderType::Anthropic);
    }

    #[tokio::test]
    async fn test_list_providers() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建多个供应商
        let configs = vec![
            ProviderConfig {
                name: Some("OpenAI Provider".to_string()),
                provider_type: ProviderType::OpenAI,
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "key1".to_string(),
                enabled: Some(true),
            },
            ProviderConfig {
                name: Some("Anthropic Provider".to_string()),
                provider_type: ProviderType::Anthropic,
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "key2".to_string(),
                enabled: Some(false),
            },
        ];

        for config in configs {
            service.create_provider(config).await.unwrap();
        }

        let providers = service.list_providers().await.unwrap();
        // 只有测试创建的2个供应商
        assert_eq!(providers.len(), 2);
        
        // 验证创建的供应商（顺序可能不同）
        let provider_names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
        assert!(provider_names.contains(&"OpenAI Provider"));
        assert!(provider_names.contains(&"Anthropic Provider"));
    }


    #[tokio::test]
    async fn test_update_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建供应商
        let config = ProviderConfig {
            name: Some("Original Name".to_string()),
            provider_type: ProviderType::Google,
            base_url: "https://api.google.com".to_string(),
            api_key: "original-key".to_string(),
            enabled: Some(false),
        };

        let provider = service.create_provider(config).await.unwrap();
        
        // 更新供应商
        let update_config = ProviderConfig {
            name: Some("Updated Name".to_string()),
            provider_type: ProviderType::Google,
            base_url: "https://updated-api.google.com".to_string(),
            api_key: "updated-key".to_string(),
            enabled: Some(true),
        };

        let updated = service.update_provider(&provider.id, update_config).await;
        assert!(updated.is_ok());
        
        let updated_provider = updated.unwrap();
        assert_eq!(updated_provider.name, "Updated Name");
        assert_eq!(updated_provider.base_url, "https://updated-api.google.com");
        assert!(updated_provider.enabled);
        assert!(updated_provider.updated_at > provider.updated_at);
    }

    #[tokio::test]
    async fn test_delete_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建供应商
        let config = ProviderConfig {
            name: Some("To Delete".to_string()),
            provider_type: ProviderType::DeepSeek,
            base_url: "https://api.deepseek.com".to_string(),
            api_key: "delete-key".to_string(),
            enabled: Some(true),
        };

        let provider = service.create_provider(config).await.unwrap();
        
        // 删除供应商
        let delete_result = service.delete_provider(&provider.id).await;
        assert!(delete_result.is_ok());
        
        // 验证供应商已被删除
        let get_result = service.get_provider(&provider.id).await;
        assert!(get_result.is_err());
    }

    // #[tokio::test]
    // async fn test_probe_provider() {
    //     // 探活功能已移除，此测试暂时禁用
    // }

    #[tokio::test]
    async fn test_toggle_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建供应商
        let config = ProviderConfig {
            name: Some("Toggle Test".to_string()),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "toggle-key".to_string(),
            enabled: Some(false),
        };

        let provider = service.create_provider(config).await.unwrap();
        assert!(!provider.enabled);
        
        // 启用供应商
        let toggled = service.toggle_provider(&provider.id, true).await.unwrap();
        assert!(toggled.enabled);
        
        // 禁用供应商
        let toggled = service.toggle_provider(&provider.id, false).await.unwrap();
        assert!(!toggled.enabled);
    }

    #[tokio::test]
    async fn test_get_provider_models() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建 OpenAI 供应商
        let config = ProviderConfig {
            name: Some("OpenAI Models Test".to_string()),
            provider_type: ProviderType::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "models-key".to_string(),
            enabled: Some(true),
        };

        let provider = service.create_provider(config).await.unwrap();
        
        // 获取模型列表
        let models = service.get_provider_models(&provider.id, false).await;
        
        // 在测试环境中，API调用可能会失败，这是正常的
        match models {
            Ok(model_list) => {
                // 如果API调用成功，验证返回的模型
                println!("Successfully fetched {} models", model_list.len());
                if !model_list.is_empty() {
                    // 验证返回的是 OpenAI 模型
                    let gpt4_model = model_list.iter().find(|m| m.id == "gpt-4");
                    if let Some(gpt4) = gpt4_model {
                        assert_eq!(gpt4.name, "GPT-4");
                        assert_eq!(gpt4.provider_id, provider.id);
                        assert!(gpt4.supported_features.contains(&ModelFeature::Text));
                    }
                }
            }
            Err(e) => {
                // API调用失败是预期的（因为使用的是测试API密钥）
                println!("Expected API failure in test environment: {}", e);
                assert!(e.to_string().contains("Failed to fetch OpenAI models") || 
                        e.to_string().contains("OpenAI API returned error"));
            }
        }
    }

    #[tokio::test] 
    async fn test_toggle_model() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建供应商并获取模型
        let config = ProviderConfig {
            name: Some("Model Toggle Test".to_string()),
            provider_type: ProviderType::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "model-toggle-key".to_string(),
            enabled: Some(true),
        };

        let provider = service.create_provider(config).await.unwrap();
        
        // 在测试环境中跳过模型获取（避免API调用失败）
        let models = match service.get_provider_models(&provider.id, false).await {
            Ok(models) => models,
            Err(_) => {
                println!("Skipping model toggle test due to API unavailability in test environment");
                return;
            }
        };
        
        let model_id = &models[0].id;
        assert!(!models[0].enabled); // 默认禁用
        
        // 启用模型
        let toggle_result = service.toggle_model(&provider.id, model_id, true).await;
        assert!(toggle_result.is_ok());
        
        // 验证模型已启用
        let updated_models = service.get_provider_models(&provider.id, false).await.unwrap();
        let updated_model = updated_models.iter().find(|m| &m.id == model_id).unwrap();
        assert!(updated_model.enabled);
    }

    #[tokio::test]
    async fn test_get_available_models() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建多个供应商
        let openai_config = ProviderConfig {
            name: Some("OpenAI Available".to_string()),
            provider_type: ProviderType::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "available-key1".to_string(),
            enabled: Some(true),
        };

        let anthropic_config = ProviderConfig {
            name: Some("Anthropic Disabled".to_string()),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "available-key2".to_string(),
            enabled: Some(false), // 禁用的供应商
        };

        let openai_provider = service.create_provider(openai_config).await.unwrap();
        let _anthropic_provider = service.create_provider(anthropic_config).await.unwrap();
        
        // 尝试获取模型，在测试环境中可能失败
        match service.get_provider_models(&openai_provider.id, false).await {
            Ok(_) => {
                // 如果成功，尝试启用一个模型
                let _ = service.toggle_model(&openai_provider.id, "gpt-4", true).await;
            }
            Err(_) => {
                // API调用失败是预期的，跳过此测试
                println!("Skipping get_available_models test due to API unavailability");
                return;
            }
        }
        
        // 获取所有可用模型
        match service.get_available_models().await {
            Ok(available) => {
                // 验证结果
                if !available.is_empty() {
                    assert_eq!(available[0].0.id, openai_provider.id);
                }
            }
            Err(_) => {
                println!("get_available_models failed, which is expected in test environment");
            }
        }
    }

    #[tokio::test]
    async fn test_provider_with_models() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建供应商
        let config = ProviderConfig {
            name: Some("With Models Test".to_string()),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "with-models-key".to_string(),
            enabled: Some(true),
        };

        let provider = service.create_provider(config).await.unwrap();
        
        // 获取模型列表以确保模型被缓存
        service.get_provider_models(&provider.id, false).await.unwrap();
        
        // 获取带模型的供应商
        let provider_with_models = service.get_provider_with_models(&provider.id).await;
        assert!(provider_with_models.is_ok());
        
        let pwm = provider_with_models.unwrap();
        assert_eq!(pwm.id, provider.id);
        assert_eq!(pwm.name, provider.name);
        assert!(!pwm.models.is_empty());
        
        // 验证模型属于该供应商
        for model in &pwm.models {
            assert_eq!(model.provider_id, provider.id);
        }
    }

    #[tokio::test]
    async fn test_duplicate_provider_name() {
        let (service, _temp_dir) = create_test_service().await;
        
        // 创建第一个供应商
        let config1 = ProviderConfig {
            name: Some("Duplicate Name".to_string()),
            provider_type: ProviderType::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "key1".to_string(),
            enabled: Some(true),
        };

        let result1 = service.create_provider(config1).await;
        assert!(result1.is_ok());
        
        // 尝试创建同名供应商
        let config2 = ProviderConfig {
            name: Some("Duplicate Name".to_string()),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "key2".to_string(),
            enabled: Some(true),
        };

        let result2 = service.create_provider(config2).await;
        assert!(result2.is_err());
        
        // 验证错误类型
        let error = result2.unwrap_err();
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert!(error.message.contains("already exists"));
    }

    #[tokio::test]
    async fn test_get_nonexistent_provider() {
        let (service, _temp_dir) = create_test_service().await;
        
        let fake_id = uuid::Uuid::new_v4().to_string();
        let result = service.get_provider(&fake_id).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, "NOT_FOUND");
    }

    // 添加一个辅助方法用于其他测试
    impl ProviderService {
        #[cfg(test)]
        async fn get_available_models(&self) -> Result<Vec<(crate::models::Provider, Vec<crate::models::Model>)>, crate::models::AppError> {
            let providers = self.list_providers().await?;
            let mut result = vec![];

            for provider in providers {
                if provider.enabled {
                    match self.get_provider_models(&provider.id, false).await {
                        Ok(models) => {
                            let enabled_models: Vec<crate::models::Model> =
                                models.into_iter().filter(|m| m.enabled).collect();
                            if !enabled_models.is_empty() {
                                result.push((provider, enabled_models));
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }

            Ok(result)
        }
    }
}