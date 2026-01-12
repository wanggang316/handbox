use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FavoriteMessageType {
    Text,
    Image,
    Message,
    Chat,
    Other,
}

impl Default for FavoriteMessageType {
    fn default() -> Self {
        FavoriteMessageType::Message
    }
}

impl FavoriteMessageType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "text" => FavoriteMessageType::Text,
            "image" => FavoriteMessageType::Image,
            "chat" => FavoriteMessageType::Chat,
            "other" => FavoriteMessageType::Other,
            _ => FavoriteMessageType::Message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: UUID,
    pub message_id: UUID,
    pub chat_id: UUID,
    pub content: String,
    pub role: String,
    pub message_type: FavoriteMessageType,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: Timestamp,
}

impl Favorite {
    pub fn tags_to_json(&self) -> Option<String> {
        if self.tags.is_empty() {
            None
        } else {
            serde_json::to_string(&self.tags).ok()
        }
    }

    pub fn tags_from_json(json: Option<&str>) -> Vec<String> {
        match json {
            Some(s) => serde_json::from_str(s).unwrap_or_default(),
            None => vec![],
        }
    }
}

pub struct CreateFavoriteRequest {
    pub message_id: UUID,
    pub chat_id: UUID,
    pub content: String,
    pub role: String,
    pub message_type: FavoriteMessageType,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub created_at: i64,
}
