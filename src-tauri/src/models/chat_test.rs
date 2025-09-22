#[cfg(test)]
mod tests {
    use crate::models::{Chat, ModelParameters};
    use serde_json;

    #[test]
    fn test_chat_serialization() {
        let chat = Chat {
            id: "chat_123".to_string(),
            name: "Test Chat".to_string(),
            last_message_at: Some(1000),
            message_count: 5,
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: vec!["server1".to_string()],
            artifact_id: None,
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&chat).expect("Failed to serialize chat");
        let deserialized: Chat = serde_json::from_str(&json).expect("Failed to deserialize chat");

        assert_eq!(chat.id, deserialized.id);
        assert_eq!(chat.name, deserialized.name);
        assert_eq!(chat.message_count, deserialized.message_count);
    }

    #[test]
    fn test_model_parameters_default() {
        let params = ModelParameters::default();
        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.max_tokens, Some(2048));
        assert_eq!(params.context_length, Some(4096));
        assert_eq!(params.stream, Some(true));
    }
}
