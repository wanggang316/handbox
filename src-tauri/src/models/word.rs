// 单词相关请求类型

use serde::{Deserialize, Serialize};
use crate::storage::types::{Word, WordContext, WordReview};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWordContextRequest {
    pub context_text: String,
    pub source_type: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWordRequest {
    pub term: String,
    pub language: String,
    pub translation: String,
    pub phonetic: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: String,
    pub context: Option<CreateWordContextRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWordRequest {
    pub id: String,
    pub term: Option<String>,
    pub language: Option<String>,
    pub translation: Option<String>,
    pub phonetic: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWordsRequest {
    pub query: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewWordRequest {
    pub word_id: String,
    pub remembered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateWordRequest {
    pub term: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateWordResponse {
    pub term: String,
    pub translation: String,
    pub target_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordDetail {
    pub word: Word,
    pub contexts: Vec<WordContext>,
    pub review: Option<WordReview>,
}
