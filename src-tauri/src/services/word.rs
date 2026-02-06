// 单词业务逻辑

use crate::models::{
    AppError, CreateWordRequest, UpdateWordRequest,
};
use crate::services::SettingsService;
use crate::storage::types::{Timestamp, Word, UUID};
use crate::storage::WordRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct WordService {
    repo: Arc<WordRepository>,
    #[allow(dead_code)]
    settings_service: SettingsService,
}

impl WordService {
    pub fn new(
        repo: Arc<WordRepository>,
        settings_service: SettingsService,
    ) -> Self {
        Self {
            repo,
            settings_service,
        }
    }

    pub async fn create_word(&self, request: CreateWordRequest) -> Result<Word, AppError> {
        if request.term.trim().is_empty() {
            return Err(AppError::validation_error("单词不能为空"));
        }

        let now = now_millis();
        let word = Word {
            id: uuid::Uuid::new_v4().to_string(),
            term: request.term,
            language: request.language,
            translation: request.translation,
            phonetic: request.phonetic,
            explanation: request.explanation,
            note: request.note,
            tags: request.tags.unwrap_or_default(),
            source: request.source,
            created_at: now,
            updated_at: now,
        };

        self.repo.create_word(&word).await?;
        Ok(word)
    }

    pub async fn list_words(
        &self,
        query: Option<String>,
        tag: Option<String>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Word>, AppError> {
        self.repo.list_words(query, tag, limit, offset).await
    }

    pub async fn get_word(&self, word_id: &UUID) -> Result<Word, AppError> {
        self.repo
            .get_word(word_id)
            .await?
            .ok_or_else(|| AppError::not_found("单词不存在"))
    }

    pub async fn update_word(&self, request: UpdateWordRequest) -> Result<Word, AppError> {
        let mut word = self
            .repo
            .get_word(&request.id)
            .await?
            .ok_or_else(|| AppError::not_found("单词不存在"))?;

        if let Some(term) = request.term {
            word.term = term;
        }
        if let Some(language) = request.language {
            word.language = language;
        }
        if let Some(translation) = request.translation {
            word.translation = translation;
        }
        if let Some(phonetic) = request.phonetic {
            word.phonetic = Some(phonetic);
        }
        if let Some(explanation) = request.explanation {
            word.explanation = Some(explanation);
        }
        if let Some(note) = request.note {
            word.note = Some(note);
        }
        if let Some(tags) = request.tags {
            word.tags = tags;
        }
        if let Some(source) = request.source {
            word.source = source;
        }

        word.updated_at = now_millis();
        self.repo.update_word(&word).await?;

        Ok(word)
    }

    pub async fn delete_word(&self, word_id: &UUID) -> Result<(), AppError> {
        self.repo.delete_word(word_id).await
    }
}

fn now_millis() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
