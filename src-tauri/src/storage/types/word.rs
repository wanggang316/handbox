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
