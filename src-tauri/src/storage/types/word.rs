use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub id: UUID,
    pub term: String,
    pub language: String,
    pub translation: String,
    pub phonetic: Option<String>,
    pub explanation: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub source: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordContext {
    pub id: UUID,
    pub word_id: UUID,
    pub context_text: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordReview {
    pub word_id: UUID,
    pub ease: f32,
    pub interval_days: i32,
    pub next_review_at: Timestamp,
    pub last_reviewed_at: Option<Timestamp>,
    pub review_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordLookupHistory {
    pub id: UUID,
    pub term: String,
    pub translation: Option<String>,
    pub phonetic: Option<String>,
    pub explanation: Option<String>,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub created_at: Timestamp,
}
