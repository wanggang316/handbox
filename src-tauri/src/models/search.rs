use crate::storage::types::UUID;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchItemType {
    Message,
    Chat,
    Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortBy {
    Relevance,
    Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub types: Option<Vec<SearchItemType>>,
    #[serde(default)]
    pub chat_id: Option<UUID>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub sort_by: Option<SearchSortBy>,
    #[serde(default)]
    pub sort_order: Option<SearchSortOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: UUID,
    #[serde(rename = "type")]
    pub result_type: SearchItemType,
    pub title: String,
    pub content: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<UUID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<UUID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<UUID>,
    pub score: f32,
    pub timestamp: i64,
    #[serde(default)]
    pub highlights: Vec<HighlightRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: i64,
    pub query: String,
    pub took: i64,
}
