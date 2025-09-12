#[cfg(test)]
mod tests {
    use crate::models::{
        Chat, ChatMessage, ChatRequest, ChatResponse, Message, MessageAttachment, MessageConfig,
        MessageRole, ModelParameters,
    };
    use serde_json;

    #[test]
    fn test_message_serialization() {
        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: MessageRole::User,
            content: "Hello world".to_string(),
            config: Some(MessageConfig {
                temperature: Some(0.7),
                top_p: Some(0.9),
                max_tokens: Some(2048),
                stream: Some(true),
                model_id: Some("gpt-4".to_string()),
                provider_id: Some("openai".to_string()),
                system_prompt: None,
                mcp_servers: None,
            }),
            attachments: None,
            input_tokens: Some(100),
            output_tokens: Some(200),
            total_tokens: Some(300),
            start_time: Some(1693456789000),
            end_time: Some(1693456790000),
            duration: Some(1000),
            created_at: 1693456789000,
            updated_at: 1693456789000,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize message");
        let deserialized: Message =
            serde_json::from_str(&json).expect("Failed to deserialize message");

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.chat_id, deserialized.chat_id);
        assert_eq!(message.config, deserialized.config);
    }

    #[test]
    fn test_chat_serialization() {
        let chat = Chat {
            id: "chat_123".to_string(),
            name: "Test Chat".to_string(),
            last_message_at: Some(1693456789000),
            message_count: 5,
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: vec!["server1".to_string(), "server2".to_string()],
            artifact_id: Some("artifact_456".to_string()),
            created_at: 1693456789000,
            updated_at: 1693456789000,
        };

        let json = serde_json::to_string(&chat).expect("Failed to serialize chat");
        let deserialized: Chat = serde_json::from_str(&json).expect("Failed to deserialize chat");

        assert_eq!(chat.id, deserialized.id);
        assert_eq!(chat.name, deserialized.name);
        assert_eq!(chat.system_prompt, deserialized.system_prompt);
        assert_eq!(chat.mcp_servers, deserialized.mcp_servers);
    }

    #[test]
    fn test_chat_request_with_model_info() {
        let request = ChatRequest {
            chat_id: Some("chat_123".to_string()),
            artifact_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            parameters: Some(ModelParameters::default()),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                reasoning: None,
            }],
            attachments: None,
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize chat request");
        let deserialized: ChatRequest =
            serde_json::from_str(&json).expect("Failed to deserialize chat request");

        assert_eq!(request.model_id, deserialized.model_id);
        assert_eq!(request.provider_id, deserialized.provider_id);
        assert_eq!(request.chat_id, deserialized.chat_id);
    }

    #[test]
    fn test_chat_response_with_metrics() {
        let response = ChatResponse {
            chat_id: "chat_123".to_string(),
            message_id: "msg_456".to_string(),
            content: "Hello! How can I help you?".to_string(),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            input_tokens: Some(10),
            output_tokens: Some(25),
            total_tokens: Some(35),
            duration: Some(1500),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize chat response");
        let deserialized: ChatResponse =
            serde_json::from_str(&json).expect("Failed to deserialize chat response");

        assert_eq!(response.model_id, deserialized.model_id);
        assert_eq!(response.provider_id, deserialized.provider_id);
        assert_eq!(response.input_tokens, deserialized.input_tokens);
        assert_eq!(response.output_tokens, deserialized.output_tokens);
        assert_eq!(response.total_tokens, deserialized.total_tokens);
        assert_eq!(response.duration, deserialized.duration);
    }

    #[test]
    fn test_message_role_serialization() {
        let roles = vec![
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::System,
        ];

        for role in roles {
            let json = serde_json::to_string(&role).expect("Failed to serialize role");
            let deserialized: MessageRole =
                serde_json::from_str(&json).expect("Failed to deserialize role");

            match role {
                MessageRole::User => assert!(matches!(deserialized, MessageRole::User)),
                MessageRole::Assistant => assert!(matches!(deserialized, MessageRole::Assistant)),
                MessageRole::System => assert!(matches!(deserialized, MessageRole::System)),
            }
        }
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

    #[test]
    fn test_message_attachment_serialization() {
        let attachment = MessageAttachment {
            id: "att_123".to_string(),
            name: "document.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            size: 1024000,
            path: "/uploads/document.pdf".to_string(),
        };

        let json = serde_json::to_string(&attachment).expect("Failed to serialize attachment");
        let deserialized: MessageAttachment =
            serde_json::from_str(&json).expect("Failed to deserialize attachment");

        assert_eq!(attachment.id, deserialized.id);
        assert_eq!(attachment.mime_type, deserialized.mime_type);
        assert_eq!(attachment.size, deserialized.size);
    }
}
