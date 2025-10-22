use crate::models::{AppError, SearchItemType, SearchRequest, SearchResponse, SearchResult};
use crate::services::StorageService;
use crate::storage::Database;
use serde_json;
use sqlx::Row;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use tokio::sync::Mutex;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
const HISTORY_CAPACITY: usize = 50;

#[derive(Clone)]
pub struct SearchService {
    db: Arc<Database>,
    storage: Arc<StorageService>,
    history_lock: Arc<Mutex<()>>,
}

impl SearchService {
    pub fn new(db: Arc<Database>, storage: Arc<StorageService>) -> Self {
        Self {
            db,
            storage,
            history_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchResponse, AppError> {
        let start = Instant::now();
        let query = request.query.trim();

        if query.is_empty() {
            return Ok(SearchResponse {
                results: vec![],
                total: 0,
                query: query.to_string(),
                took: 0,
            });
        }

        if let Some(types) = &request.types {
            if !types.iter().any(|t| matches!(t, SearchItemType::Message)) {
                return Ok(SearchResponse {
                    results: vec![],
                    total: 0,
                    query: query.to_string(),
                    took: 0,
                });
            }
        }

        let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as i64;
        let offset = request.offset.unwrap_or(0).max(0) as i64;

        let mut conditions = vec!["m.role IN ('user', 'assistant')".to_string()];
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(chat_id) = &request.chat_id {
            conditions.push("m.chat_id = ?".to_string());
            bind_values.push(chat_id.clone());
        }

        let escaped = Self::escape_like_pattern(query);
        let pattern = format!("%{}%", escaped);

        conditions.push("(m.content LIKE ? ESCAPE '\\' OR (m.reasoning IS NOT NULL AND m.reasoning LIKE ? ESCAPE '\\'))".to_string());

        let mut sql = format!(
            "SELECT m.id, m.chat_id, m.content, m.created_at, c.name \
             FROM messages m \
             JOIN chats c ON c.id = m.chat_id \
             WHERE {}",
            conditions.join(" AND ")
        );

        let order_direction = match request
            .sort_order
            .unwrap_or(crate::models::SearchSortOrder::Desc)
        {
            crate::models::SearchSortOrder::Asc => "ASC",
            crate::models::SearchSortOrder::Desc => "DESC",
        };

        let order_field = match request
            .sort_by
            .unwrap_or(crate::models::SearchSortBy::Timestamp)
        {
            crate::models::SearchSortBy::Timestamp => "m.created_at",
            crate::models::SearchSortBy::Relevance => "m.created_at",
        };

        sql.push_str(&format!(
            " ORDER BY {} {} LIMIT ? OFFSET ?",
            order_field, order_direction
        ));

        let mut query_builder = sqlx::query(&sql);

        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        query_builder = query_builder.bind(&pattern);
        query_builder = query_builder.bind(&pattern);
        query_builder = query_builder.bind(limit);
        query_builder = query_builder.bind(offset);

        let rows = query_builder.fetch_all(self.db.pool()).await.map_err(|e| {
            AppError::internal_error(&format!("Failed to execute search query: {}", e))
        })?;

        let count_sql = format!(
            "SELECT COUNT(*) as total FROM messages m WHERE {}",
            conditions.join(" AND ")
        );

        let mut count_query = sqlx::query(&count_sql);

        for value in &bind_values {
            count_query = count_query.bind(value);
        }
        count_query = count_query.bind(&pattern);
        count_query = count_query.bind(&pattern);

        let total: i64 = count_query
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch search count: {}", e)))?
            .try_get("total")
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to read search count: {}", e))
            })?;

        let results = rows
            .into_iter()
            .map(|row| -> Result<SearchResult, AppError> {
                let message_id: String = row.try_get("id").map_err(|e| {
                    AppError::internal_error(&format!("Failed to read message id: {}", e))
                })?;
                let chat_id: String = row.try_get("chat_id").map_err(|e| {
                    AppError::internal_error(&format!("Failed to read chat id: {}", e))
                })?;
                let content: String = row.try_get("content").map_err(|e| {
                    AppError::internal_error(&format!("Failed to read content: {}", e))
                })?;
                let created_at: i64 = row.try_get("created_at").map_err(|e| {
                    AppError::internal_error(&format!("Failed to read timestamp: {}", e))
                })?;
                let chat_name: String = row.try_get("name").unwrap_or_else(|_| "".to_string());

                let snippet = Self::build_snippet(&content, query);

                Ok(SearchResult {
                    id: message_id.clone(),
                    result_type: SearchItemType::Message,
                    title: chat_name,
                    content: content.clone(),
                    snippet,
                    chat_id: Some(chat_id.clone()),
                    message_id: Some(message_id),
                    artifact_id: None,
                    score: 1.0,
                    timestamp: created_at,
                    highlights: vec![],
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;

        let took = start.elapsed().as_millis() as i64;

        Ok(SearchResponse {
            results,
            total,
            query: query.to_string(),
            took,
        })
    }

    pub async fn get_history(&self, limit: Option<usize>) -> Result<Vec<String>, AppError> {
        let _lock = self.history_lock.lock().await;
        let mut history = self.read_history_file().await?;
        if let Some(limit) = limit {
            history.truncate(limit);
        }
        Ok(history)
    }

    pub async fn add_history_entry(&self, entry: &str) -> Result<(), AppError> {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let _lock = self.history_lock.lock().await;
        let mut history = self.read_history_file().await?;
        history.retain(|item| item != trimmed);
        history.insert(0, trimmed.to_string());
        history.truncate(HISTORY_CAPACITY);
        self.write_history_file(&history).await
    }

    pub async fn clear_history(&self) -> Result<(), AppError> {
        let _lock = self.history_lock.lock().await;
        let path = self.storage.get_search_history_path();
        if path.exists() {
            fs::remove_file(&path).await.map_err(|e| {
                AppError::internal_error(&format!("Failed to remove history file: {}", e))
            })?;
        }
        Ok(())
    }

    pub async fn get_suggestions(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>, AppError> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(vec![]);
        }

        let _lock = self.history_lock.lock().await;
        let history = self.read_history_file().await?;
        let mut suggestions: Vec<String> = history
            .into_iter()
            .filter(|item| item.to_lowercase().contains(&needle))
            .collect();
        if let Some(limit) = limit {
            suggestions.truncate(limit);
        }
        Ok(suggestions)
    }

    fn escape_like_pattern(input: &str) -> String {
        let mut escaped = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '%' | '_' | '\\' => {
                    escaped.push('\\');
                    escaped.push(ch);
                }
                _ => escaped.push(ch),
            }
        }
        escaped
    }

    fn build_snippet(content: &str, _query: &str) -> String {
        const MAX_LENGTH: usize = 160;
        if content.chars().count() <= MAX_LENGTH {
            content.to_string()
        } else {
            let preview: String = content.chars().take(MAX_LENGTH).collect();
            format!("{}…", preview)
        }
    }

    async fn read_history_file(&self) -> Result<Vec<String>, AppError> {
        let path = self.storage.get_search_history_path();
        if !path.exists() {
            return Ok(vec![]);
        }

        let data = fs::read_to_string(&path).await.map_err(|e| {
            AppError::internal_error(&format!("Failed to read history file: {}", e))
        })?;

        let history: Vec<String> = serde_json::from_str(&data).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse history file: {}", e))
        })?;

        Ok(history)
    }

    async fn write_history_file(&self, history: &[String]) -> Result<(), AppError> {
        let path = self.storage.get_search_history_path();
        let serialized = serde_json::to_string(history).map_err(|e| {
            AppError::internal_error(&format!("Failed to serialize history file: {}", e))
        })?;
        fs::write(&path, serialized)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to write history file: {}", e)))
    }
}
