// 单词业务逻辑

use crate::models::{
    AppError, CreateWordLookupRequest, CreateWordRequest, ListWordLookupHistoryRequest,
    ReviewWordRequest, TranslateWordRequest, TranslateWordResponse, UpdateWordRequest, WordDetail,
};
use crate::services::{ProviderService, SettingsService};
use crate::storage::types::{Timestamp, Word, WordContext, WordLookupHistory, WordReview, UUID};
use crate::storage::word_repository::next_review_timestamp;
use crate::storage::WordRepository;
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::types::{LlmMessage, LlmMessageRole, LlmRequest};
use handbox_llm::{create_llm_client, LlmProvider};
use std::sync::Arc;

#[derive(Clone)]
pub struct WordService {
    repo: Arc<WordRepository>,
    provider_service: Arc<ProviderService>,
    settings_service: SettingsService,
    llm_config: Arc<dyn LlmConfigProvider>,
}

impl WordService {
    pub fn new(
        repo: Arc<WordRepository>,
        provider_service: Arc<ProviderService>,
        settings_service: SettingsService,
        llm_config: Arc<dyn LlmConfigProvider>,
    ) -> Self {
        Self {
            repo,
            provider_service,
            settings_service,
            llm_config,
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
        if request.term.trim().is_empty() {
            return Err(AppError::validation_error("请输入需要翻译的单词"));
        }

        let settings = self.settings_service.get_settings()?;
        let provider_id = settings
            .translation
            .provider_id
            .ok_or_else(|| AppError::validation_error("请先设置翻译供应商"))?;
        let model_id = settings
            .translation
            .model_id
            .ok_or_else(|| AppError::validation_error("请先设置翻译模型"))?;

        let target_language = if settings.translation.target_language == "system" {
            match settings.general.language {
                crate::models::Language::ZhCN => "zh-CN".to_string(),
                crate::models::Language::EnUS => "en-US".to_string(),
            }
        } else {
            settings.translation.target_language.clone()
        };

        let provider = self.provider_service.get_provider(&provider_id).await?;
        if provider.api_key.is_empty() {
            return Err(AppError::validation_error("翻译供应商未配置 API Key"));
        }

        let llm_client = create_llm_client(&provider.provider_type, Arc::clone(&self.llm_config))
            .map_err(|e| {
            let error: AppError = e.into();
            tracing::error!(
                "[WordService::translate_word] Failed to create LLM client: {}",
                error.message
            );
            error
        })?;

        let prompt = format!(
            "请将下面的词、短语或句子翻译为目标语言（{}）。只返回一行 JSON，不要使用 Markdown 代码块，不要输出额外文本。格式：{{\"translation\":\"...\",\"phonetic\":\"...\",\"explanation\":\"...\"}}。其中 phonetic 为源词/短语的音标（如果适用，句子可为空），explanation 为简短解释（单行，不换行）。",
            target_language
        );

        let api_request = LlmRequest {
            model: model_id.clone(),
            messages: vec![
                LlmMessage {
                    role: LlmMessageRole::System,
                    content: prompt,
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                    attachments: None,
                },
                LlmMessage {
                    role: LlmMessageRole::User,
                    content: request.term.clone(),
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                    attachments: None,
                },
            ],
            temperature: Some(0.2),
            top_p: None,
            top_k: None,
            max_tokens: Some(512),
            stream: Some(false),
            reasoning: None,
            reasoning_effort: None,
            thinking: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let provider_context = LlmProvider {
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        };

        let response = llm_client
            .chat(&provider_context, api_request)
            .await
            .map_err(|e| {
                let error: AppError = e.into();
                tracing::error!(
                    "[WordService::translate_word] LLM API call failed: {}",
                    error.message
                );
                error
            })?;

        let raw_content = extract_first_choice_content(response)?;

        let parsed = parse_translation_json(&raw_content);
        let (mut translation, mut phonetic, mut explanation) = match parsed.as_ref() {
            Some(parsed) => (
                parsed.translation.clone(),
                normalize_optional(parsed.phonetic.clone()),
                normalize_optional(parsed.explanation.clone()),
            ),
            None => {
                tracing::warn!(
                    "[WordService::translate_word] Failed to parse translation JSON: {}",
                    raw_content
                );
                (raw_content.clone(), None, None)
            }
        };

        if parsed.is_none() && looks_truncated_json(&raw_content) {
            tracing::warn!(
                "[WordService::translate_word] Detected truncated JSON, retrying with plain translation"
            );
            let fallback_prompt = format!(
                "请将下面的词、短语或句子翻译为目标语言（{}）。只返回译文，不要包含 Markdown、JSON 或解释。",
                target_language
            );
            let fallback_request = LlmRequest {
                model: model_id.clone(),
                messages: vec![
                    LlmMessage {
                        role: LlmMessageRole::System,
                        content: fallback_prompt,
                        reasoning: None,
                        tool_calls: None,
                        tool_call_id: None,
                        attachments: None,
                    },
                    LlmMessage {
                        role: LlmMessageRole::User,
                        content: request.term.clone(),
                        reasoning: None,
                        tool_calls: None,
                        tool_call_id: None,
                        attachments: None,
                    },
                ],
                temperature: Some(0.2),
                top_p: None,
                top_k: None,
                max_tokens: Some(128),
                stream: Some(false),
                reasoning: None,
                reasoning_effort: None,
                thinking: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
            };

            if let Ok(fallback_response) =
                llm_client.chat(&provider_context, fallback_request).await
            {
                if let Ok(fallback_content) = extract_first_choice_content(fallback_response) {
                    let fallback_text = fallback_content.trim();
                    if !fallback_text.is_empty() {
                        translation = fallback_text.to_string();
                        phonetic = None;
                        explanation = None;
                    }
                }
            }
        }

        Ok(TranslateWordResponse {
            term: request.term,
            translation,
            target_language,
            phonetic,
            explanation,
        })
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

#[derive(serde::Deserialize)]
struct TranslationPayload {
    translation: String,
    phonetic: Option<String>,
    explanation: Option<String>,
}

fn parse_translation_json(input: &str) -> Option<TranslationPayload> {
    let trimmed = input.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    };

    if let Ok(parsed) = serde_json::from_str::<TranslationPayload>(&json_text) {
        return Some(parsed);
    }

    // Fallback: extract the first JSON object if extra text exists.
    let start = json_text.find('{')?;
    let end = json_text.rfind('}')?;
    if start >= end {
        return None;
    }
    let slice = json_text[start..=end].trim();
    serde_json::from_str::<TranslationPayload>(slice).ok()
}

fn extract_first_choice_content(
    response: handbox_llm::types::LlmResponse,
) -> Result<String, AppError> {
    response
        .choices
        .first()
        .and_then(|choice| choice.delta.as_ref())
        .map(|message| message.content.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::internal_error("翻译结果为空"))
}

fn looks_truncated_json(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    let looks_like_json = trimmed.starts_with("```") || trimmed.starts_with('{');
    let has_open = trimmed.contains('{');
    let has_close = trimmed.contains('}');
    looks_like_json && has_open && !has_close
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
