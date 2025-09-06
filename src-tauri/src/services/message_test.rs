#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, ChatRequest, MessageRole, ModelParameters};
    use crate::services::{ChatService, DatabaseService, MessageService};
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database_service() -> Arc<DatabaseService> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let db_service = DatabaseService::new(&db_path)
            .await
            .expect("Failed to create database service");
        Arc::new(db_service)
    }

    async fn setup_test_services() -> (ChatService, MessageService, String) {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db.clone());
        let message_service = MessageService::new(db);
        
        // 创建一个测试聊天
        let chat = chat_service.create_chat("Test Chat".to_string(), None, None).await.unwrap();
        
        (chat_service, message_service, chat.id)
    }

    #[tokio::test]
    async fn test_message_service_creation() {
        let db = create_test_database_service().await;
        let message_service = MessageService::new(db);

        // 验证服务可以成功创建
        assert!(true); // 如果能到这里说明创建成功
    }

    #[tokio::test]
    async fn test_send_message() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        let request = ChatRequest {
            chat_id: Some(chat_id.clone()),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: Some(ModelParameters::default()),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hello, world!".to_string(),
            }],
            attachments: None,
        };

        let result = message_service.send_message(request).await;

        // 验证消息发送成功
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.chat_id, chat_id);
        assert!(!response.message_id.is_empty());
        assert!(!response.content.is_empty());
        assert_eq!(response.model_id, "gpt-4");
        assert_eq!(response.provider_id, "openai");
    }

    #[tokio::test]
    async fn test_send_message_validation_error() {
        let (_chat_service, message_service, _chat_id) = setup_test_services().await;

        let request = ChatRequest {
            chat_id: None, // 缺少 chat_id
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
            }],
            attachments: None,
        };

        let result = message_service.send_message(request).await;

        // 验证返回验证错误
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, "VALIDATION_ERROR");
        }
    }

    #[tokio::test]
    async fn test_get_messages() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送几条消息
        let request1 = ChatRequest {
            chat_id: Some(chat_id.clone()),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "First message".to_string(),
            }],
            attachments: None,
        };
        message_service.send_message(request1).await.unwrap();

        let request2 = ChatRequest {
            chat_id: Some(chat_id.clone()),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Second message".to_string(),
            }],
            attachments: None,
        };
        message_service.send_message(request2).await.unwrap();

        // 获取消息列表
        let result = message_service.get_messages(chat_id, Some(10), Some(0)).await;

        // 验证消息获取成功
        assert!(result.is_ok());
        let messages = result.unwrap();
        
        // 应该有 4 条消息（2 条用户消息 + 2 条助手回复）
        assert_eq!(messages.len(), 4);
        
        // 验证消息按时间排序
        assert_eq!(messages[0].content, "First message");
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[2].content, "Second message");
        assert_eq!(messages[2].role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_get_message() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送一条消息
        let request = ChatRequest {
            chat_id: Some(chat_id),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Test message".to_string(),
            }],
            attachments: None,
        };
        let response = message_service.send_message(request).await.unwrap();

        // 获取生成的助手消息
        let result = message_service.get_message(response.message_id.clone()).await;

        // 验证消息获取成功
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.id, response.message_id);
        assert_eq!(message.role, MessageRole::Assistant);
        assert!(!message.content.is_empty());
    }

    #[tokio::test]
    async fn test_get_message_not_found() {
        let (_chat_service, message_service, _chat_id) = setup_test_services().await;

        let result = message_service.get_message("nonexistent_message".to_string()).await;

        // 验证返回未找到错误
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, "NOT_FOUND");
        }
    }

    #[tokio::test]
    async fn test_update_message() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送一条消息
        let request = ChatRequest {
            chat_id: Some(chat_id),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Original content".to_string(),
            }],
            attachments: None,
        };
        let response = message_service.send_message(request).await.unwrap();

        // 更新助手消息
        let result = message_service.update_message(
            response.message_id.clone(),
            "Updated content".to_string(),
        ).await;

        // 验证消息更新成功
        assert!(result.is_ok());
        let updated_message = result.unwrap();
        assert_eq!(updated_message.id, response.message_id);
        assert_eq!(updated_message.content, "Updated content");
    }

    #[tokio::test]
    async fn test_delete_message() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送一条消息
        let request = ChatRequest {
            chat_id: Some(chat_id),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "To be deleted".to_string(),
            }],
            attachments: None,
        };
        let response = message_service.send_message(request).await.unwrap();

        // 删除助手消息
        let result = message_service.delete_message(response.message_id.clone()).await;

        // 验证消息删除成功
        assert!(result.is_ok());

        // 验证消息确实被删除了
        let get_result = message_service.get_message(response.message_id).await;
        assert!(get_result.is_err());
        if let Err(error) = get_result {
            assert_eq!(error.code, "NOT_FOUND");
        }
    }

    #[tokio::test]
    async fn test_regenerate_message() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送一条消息
        let request = ChatRequest {
            chat_id: Some(chat_id),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Test message".to_string(),
            }],
            attachments: None,
        };
        let response = message_service.send_message(request).await.unwrap();

        let original_content = response.content.clone();

        // 重新生成助手消息
        let result = message_service.regenerate_message(response.message_id.clone()).await;

        // 验证消息重新生成成功
        assert!(result.is_ok());
        let regenerated_response = result.unwrap();
        assert_eq!(regenerated_response.message_id, response.message_id);
        assert_ne!(regenerated_response.content, original_content); // 内容应该不同
        assert!(regenerated_response.content.contains("重新生成的回复"));
    }

    #[tokio::test]
    async fn test_regenerate_user_message_error() {
        let (_chat_service, message_service, chat_id) = setup_test_services().await;

        // 先发送一条消息，然后获取用户消息 ID
        let request = ChatRequest {
            chat_id: Some(chat_id.clone()),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: None,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Test message".to_string(),
            }],
            attachments: None,
        };
        message_service.send_message(request).await.unwrap();

        // 获取用户消息 ID
        let messages = message_service.get_messages(chat_id, Some(10), Some(0)).await.unwrap();
        let user_message = messages.iter().find(|m| m.role == MessageRole::User).unwrap();

        // 尝试重新生成用户消息（应该失败）
        let result = message_service.regenerate_message(user_message.id.clone()).await;

        // 验证返回验证错误
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, "VALIDATION_ERROR");
        }
    }
}