// 单词业务逻辑

use crate::models::{
    AppError, CreateWordLookupRequest, CreateWordRequest, ListWordLookupHistoryRequest,
    ReviewWordRequest, TranslateWordRequest, TranslateWordResponse, UpdateWordRequest, WordDetail,
};
use crate::services::SettingsService;
use crate::storage::types::{Timestamp, Word, WordContext, WordLookupHistory, WordReview, UUID};
use crate::storage::word_repository::next_review_timestamp;
use crate::storage::WordRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct WordService {
    repo: Arc<WordRepository>,
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

        if let Some(context) = request.context {
            let word_context = WordContext {
                id: uuid::Uuid::new_v4().to_string(),
                word_id: word.id.clone(),
                context_text: context.context_text,
                source_type: context.source_type,
                source_id: context.source_id,
                created_at: now,
            };
            self.repo.create_context(&word_context).await?;
        }

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

    pub async fn get_word_detail(&self, word_id: &UUID) -> Result<WordDetail, AppError> {
        let word = self
            .repo
            .get_word(word_id)
            .await?
            .ok_or_else(|| AppError::not_found("单词不存在"))?;

        let contexts = self.repo.list_contexts(word_id).await?;
        let review = self.repo.get_review(word_id).await?;

        Ok(WordDetail {
            word,
            contexts,
            review,
        })
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

    pub async fn review_word(&self, request: ReviewWordRequest) -> Result<WordReview, AppError> {
        let now = now_millis();
        let existing = self.repo.get_review(&request.word_id).await?;

        let mut ease = existing.as_ref().map(|r| r.ease).unwrap_or(2.5);
        let mut interval = existing.as_ref().map(|r| r.interval_days).unwrap_or(1);
        let mut review_count = existing.as_ref().map(|r| r.review_count).unwrap_or(0);

        if request.remembered {
            interval = (interval * 2).max(1);
            ease = (ease + 0.1).min(3.0);
        } else {
            interval = 1;
            ease = (ease - 0.2).max(1.3);
        }

        review_count += 1;
        let review = WordReview {
            word_id: request.word_id,
            ease,
            interval_days: interval,
            next_review_at: next_review_timestamp(now, interval),
            last_reviewed_at: Some(now),
            review_count,
        };

        self.repo.upsert_review(&review).await?;
        Ok(review)
    }

    pub async fn translate_word(
        &self,
        request: TranslateWordRequest,
    ) -> Result<TranslateWordResponse, AppError> {
        // 此方法已废弃，请使用 Session 的 sendUserMessageStream 接口进行翻译
        Err(AppError::validation_error(
            "翻译功能已迁移到 Session 模式，请在单词本页面选择翻译 Agent 和模型",
        ))
    }

    pub async fn record_lookup_history(
        &self,
        request: CreateWordLookupRequest,
    ) -> Result<WordLookupHistory, AppError> {
        if request.term.trim().is_empty() {
            return Err(AppError::validation_error("查询内容不能为空"));
        }

        let now = now_millis();
        let history = WordLookupHistory {
            id: uuid::Uuid::new_v4().to_string(),
            term: request.term,
            translation: request.translation,
            phonetic: request.phonetic,
            explanation: request.explanation,
            source_language: request.source_language,
            target_language: request.target_language,
            created_at: now,
        };

        self.repo.create_lookup_history(&history).await?;
        Ok(history)
    }

    pub async fn list_lookup_history(
        &self,
        request: ListWordLookupHistoryRequest,
    ) -> Result<Vec<WordLookupHistory>, AppError> {
        let limit = request.limit.unwrap_or(20);
        let offset = request.offset.unwrap_or(0);
        self.repo.list_lookup_history(limit, offset).await
    }

    pub async fn delete_lookup_history(&self, history_id: &UUID) -> Result<(), AppError> {
        self.repo.delete_lookup_history(history_id).await
    }
}

fn now_millis() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
