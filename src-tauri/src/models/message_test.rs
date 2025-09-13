#[cfg(test)]
mod tests {
    use crate::models::{
        Message, MessageAttachment, MessageConfig, MessageRequest, MessageRequestAttachment, MessageResponse, MessageRole,
    };
    use serde_json;

    #[test]
    fn test_message_serialization() {
        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
            reasoning: None,
            config: None,
            attachments: None,
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
            start_time: Some(1000),
            end_time: Some(2000),
            duration: Some(1000),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize message");
        let deserialized: Message =
            serde_json::from_str(&json).expect("Failed to deserialize message");

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.chat_id, deserialized.chat_id);
        assert_eq!(message.content, deserialized.content);
    }

    #[test]
    fn test_message_with_attachments() {
        let attachment = MessageAttachment {
            id: "att_123".to_string(),
            name: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 1024,
            path: "/tmp/test.txt".to_string(),
        };

        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: MessageRole::User,
            content: "Here's a file".to_string(),
            reasoning: None,
            config: None,
            attachments: Some(vec![attachment]),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: 1000,
            updated_at: 1000,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize message");
        let deserialized: Message =
            serde_json::from_str(&json).expect("Failed to deserialize message");

        assert_eq!(message.attachments.unwrap().len(), 1);
        assert_eq!(deserialized.attachments.unwrap().len(), 1);
    }

    #[test]
    fn test_message_config() {
        let config = MessageConfig {
            temperature: Some(0.8),
            top_p: Some(0.9),
            max_tokens: Some(1000),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: Some(vec!["server1".to_string()]),
        };

        let json = serde_json::to_string(&config).expect("Failed to serialize config");
        let deserialized: MessageConfig =
            serde_json::from_str(&json).expect("Failed to deserialize config");

        assert_eq!(config.temperature, deserialized.temperature);
        assert_eq!(config.model_id, deserialized.model_id);
    }

    #[test]
    fn test_message_response_with_metrics() {
        let response = MessageResponse {
            chat_id: "chat_123".to_string(),
            message_id: "msg_456".to_string(),
            content: "Hello! How can I help you?".to_string(),
            reasoning: Some("I need to be helpful and friendly".to_string()),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            input_tokens: Some(15),
            output_tokens: Some(20),
            total_tokens: Some(35),
            duration: Some(1500),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize message response");
        let deserialized: MessageResponse =
            serde_json::from_str(&json).expect("Failed to deserialize message response");

        assert_eq!(response.model_id, deserialized.model_id);
        assert_eq!(response.provider_id, deserialized.provider_id);
        assert_eq!(response.input_tokens, deserialized.input_tokens);
        assert_eq!(response.output_tokens, deserialized.output_tokens);
        assert_eq!(response.total_tokens, deserialized.total_tokens);
        assert_eq!(response.reasoning, deserialized.reasoning);
    }

    #[test]
    fn test_message_request_serialization() {
        let request = MessageRequest {
            chat_id: Some("chat_123".to_string()),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            messages: vec![],
            attachments: None,
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize message request");
        let deserialized: MessageRequest =
            serde_json::from_str(&json).expect("Failed to deserialize message request");

        assert_eq!(request.chat_id, deserialized.chat_id);
        assert_eq!(request.model_id, deserialized.model_id);
        assert_eq!(request.provider_id, deserialized.provider_id);
    }

    #[test]
    fn test_message_request_attachment() {
        let attachment = MessageRequestAttachment {
            name: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            data: b"Hello, world!".to_vec(),
        };

        let request = MessageRequest {
            chat_id: Some("chat_123".to_string()),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            messages: vec![],
            attachments: Some(vec![attachment]),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize message request");
        let deserialized: MessageRequest =
            serde_json::from_str(&json).expect("Failed to deserialize message request");

        assert_eq!(request.attachments.unwrap().len(), 1);
        assert_eq!(deserialized.attachments.unwrap().len(), 1);
    }
}
