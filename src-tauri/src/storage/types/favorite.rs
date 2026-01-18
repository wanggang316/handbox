use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FavoriteMessageType {
    Text,
    Image,
    Message,
    Chat,
    External,
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
            "external" => FavoriteMessageType::External,
            _ => FavoriteMessageType::Message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteTag {
    pub name: String,
    pub color: String,
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
    pub tags: Vec<FavoriteTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_text_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_pid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_window_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tab_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_rect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_language: Option<String>,
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

    pub fn tags_from_json(json: Option<&str>) -> Vec<FavoriteTag> {
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
    pub tags: Vec<FavoriteTag>,
    pub note: Option<String>,
    pub context: Option<String>,
    pub selection_text_raw: Option<String>,
    pub source_app_name: Option<String>,
    pub source_bundle_id: Option<String>,
    pub source_pid: Option<i64>,
    pub source_app_path: Option<String>,
    pub source_app_version: Option<String>,
    pub source_window_title: Option<String>,
    pub source_url: Option<String>,
    pub source_domain: Option<String>,
    pub source_tab_title: Option<String>,
    pub selection_rect: Option<String>,
    pub capture_method: Option<String>,
    pub locale: Option<String>,
    pub input_language: Option<String>,
    pub created_at: i64,
}
