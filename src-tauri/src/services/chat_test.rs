#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, ChatRequest, MessageRole, ModelParameters};
    use crate::services::{ChatService, DatabaseService};
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

    #[tokio::test]
    async fn test_chat_service_creation() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        // 验证服务可以成功创建
        assert!(true); // 如果能到这里说明创建成功
    }

    #[tokio::test]
    async fn test_create_chat() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        let result = chat_service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.7),                             // temperature
                Some(0.9),                             // top_p
                Some(2048),                            // max_tokens
                Some(true),                            // stream
                Some("gpt-4o".to_string()),            // model_id
                Some("openai".to_string()),            // provider_id
                Some("System prompt".to_string()),     // system_prompt
                Some(vec!["server1".to_string()]),     // mcp_servers
            )
            .await;

        // 验证聊天创建成功
        assert!(result.is_ok());
        let chat = result.unwrap();
        assert_eq!(chat.name, "Test Chat");
        assert_eq!(chat.temperature, Some(0.7));
        assert_eq!(chat.top_p, Some(0.9));
        assert_eq!(chat.max_tokens, Some(2048));
        assert_eq!(chat.stream, Some(true));
        assert_eq!(chat.model_id, Some("gpt-4o".to_string()));
        assert_eq!(chat.provider_id, Some("openai".to_string()));
        assert_eq!(chat.system_prompt, Some("System prompt".to_string()));
        assert_eq!(chat.mcp_servers, vec!["server1".to_string()]);
        assert_eq!(chat.message_count, 0);
        assert!(chat.last_message_at.is_none());
        assert!(!chat.id.is_empty());
    }

    #[tokio::test]
    async fn test_list_chats() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        // 首先创建几个聊天
        let _chat1 = chat_service.create_chat(
            "Chat 1".to_string(),
            None, None, None, None, None, None, None, None
        ).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // 确保时间戳不同
        let _chat2 = chat_service.create_chat(
            "Chat 2".to_string(),
            None, None, None, None, None, None, None, None
        ).await.unwrap();

        let result = chat_service.list_chats(Some(10), Some(0)).await;

        // 验证聊天列表获取成功
        assert!(result.is_ok());
        let chats = result.unwrap();
        assert_eq!(chats.len(), 2);
        
        // 验证聊天按更新时间排序（最新的在前，因为 SQL 用的是 DESC）
        assert_eq!(chats[0].name, "Chat 2");
        assert_eq!(chats[1].name, "Chat 1");
    }

    #[tokio::test]
    async fn test_get_chat() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        // 先创建一个聊天
        let created_chat = chat_service.create_chat(
            "Test Chat".to_string(),
            None, None, None, None, None, None, None, None
        ).await.unwrap();

        let result = chat_service.get_chat(created_chat.id.clone()).await;

        // 验证聊天获取成功
        assert!(result.is_ok());
        let chat = result.unwrap();
        assert_eq!(chat.id, created_chat.id);
        assert_eq!(chat.name, "Test Chat");
    }

    #[tokio::test]
    async fn test_get_chat_not_found() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        let result = chat_service.get_chat("nonexistent_chat".to_string()).await;

        // 验证返回未找到错误
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, "NOT_FOUND");
        }
    }

    #[tokio::test]
    async fn test_update_chat() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        // 先创建一个聊天
        let created_chat = chat_service.create_chat(
            "Original Name".to_string(),
            None, None, None, None, None, None, None, None
        ).await.unwrap();

        let result = chat_service
            .update_chat(
                created_chat.id.clone(),
                Some("Updated Name".to_string()),
                Some(0.8),                              // temperature
                Some(0.95),                             // top_p
                Some(4096),                             // max_tokens
                Some(false),                            // stream
                Some("claude-3".to_string()),           // model_id
                Some("anthropic".to_string()),          // provider_id
                Some("Updated prompt".to_string()),
                Some(vec!["server1".to_string(), "server2".to_string()]),
            )
            .await;

        // 验证聊天更新成功
        assert!(result.is_ok());
        let updated_chat = result.unwrap();
        assert_eq!(updated_chat.name, "Updated Name");
        assert_eq!(updated_chat.temperature, Some(0.8));
        assert_eq!(updated_chat.top_p, Some(0.95));
        assert_eq!(updated_chat.max_tokens, Some(4096));
        assert_eq!(updated_chat.stream, Some(false));
        assert_eq!(updated_chat.model_id, Some("claude-3".to_string()));
        assert_eq!(updated_chat.provider_id, Some("anthropic".to_string()));
        assert_eq!(updated_chat.system_prompt, Some("Updated prompt".to_string()));
        assert_eq!(updated_chat.mcp_servers, vec!["server1".to_string(), "server2".to_string()]);
        assert_eq!(updated_chat.id, created_chat.id);
    }

    #[tokio::test]
    async fn test_delete_chat() {
        let db = create_test_database_service().await;
        let chat_service = ChatService::new(db);

        // 先创建一个聊天
        let created_chat = chat_service.create_chat(
            "Test Chat".to_string(),
            None, None, None, None, None, None, None, None
        ).await.unwrap();

        let result = chat_service.delete_chat(created_chat.id.clone()).await;

        // 验证聊天删除成功
        assert!(result.is_ok());

        // 验证聊天确实被删除了
        let get_result = chat_service.get_chat(created_chat.id).await;
        assert!(get_result.is_err());
        if let Err(error) = get_result {
            assert_eq!(error.code, "NOT_FOUND");
        }
    }

}
