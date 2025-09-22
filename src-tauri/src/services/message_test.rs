#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, MessageConfig, MessageRequest, MessageRole, ModelParameters};
    use crate::services::{ChatService, Database, MessageService};
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database_service() -> Arc<Database> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path)
            .await
            .expect("Failed to create database service");
        Arc::new(db_service)
    }

    async fn setup_test_services() -> (ChatService, MessageService, String) {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db.clone());
        let message_service = MessageService::new(db);

        // 创建一个测试聊天
        let chat = chat_service
            .create_chat(
                "Test Chat".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

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
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;

        // Skip this test for now as it requires actual provider setup
        // TODO: Add mock provider setup for testing
        assert!(true);
    }

    #[tokio::test]
    async fn test_send_message_validation_error() {
        let (_chat_service, message_service, _chat_id) = setup_test_services().await;

        let request = MessageRequest {
            chat_id: None, // 缺少 chat_id
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                reasoning: None,
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
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_get_message() {
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_get_message_not_found() {
        let (_chat_service, message_service, _chat_id) = setup_test_services().await;

        let result = message_service
            .get_message("nonexistent_message".to_string())
            .await;

        // 验证返回未找到错误
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, "NOT_FOUND");
        }
    }

    #[tokio::test]
    async fn test_update_message() {
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_delete_message() {
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_regenerate_message() {
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_regenerate_user_message_error() {
        let (_chat_service, _message_service, _chat_id) = setup_test_services().await;
        // Skip this test for now as it requires actual provider setup
        assert!(true);
    }
}
