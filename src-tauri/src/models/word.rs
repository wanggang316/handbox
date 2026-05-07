// 单词相关请求类型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWordRequest {
    pub term: String,
    pub language: String,
    pub translation: String,
    pub phonetic: Option<String>,
    pub explanation: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWordRequest {
    pub id: String,
    pub term: Option<String>,
    pub language: Option<String>,
    pub translation: Option<String>,
    pub phonetic: Option<String>,
    pub explanation: Option<String>,
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

