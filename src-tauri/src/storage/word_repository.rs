// 单词数据访问层

use crate::models::AppError;
use crate::storage::types::{Word, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

#[derive(Clone)]
pub struct WordRepository {
    db: Arc<Database>,
}

impl WordRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn create_word(&self, word: &Word) -> Result<(), AppError> {
        let tags_json = serde_json::to_string(&word.tags)
            .map_err(|e| AppError::validation_error(&format!("Invalid tags: {e}")))?;

        let query = r#"
            INSERT INTO words (id, term, language, translation, phonetic, explanation, note, tags, source, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#;

        sqlx::query(query)
            .bind(&word.id)
            .bind(&word.term)
            .bind(&word.language)
            .bind(&word.translation)
            .bind(&word.phonetic)
            .bind(&word.explanation)
            .bind(&word.note)
            .bind(&tags_json)
            .bind(&word.source)
            .bind(word.created_at)
            .bind(word.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create word: {e}")))?;

        Ok(())
    }


    pub async fn list_words(
        &self,
        query: Option<String>,
        tag: Option<String>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Word>, AppError> {
        let mut sql = String::from(
            "SELECT id, term, language, translation, phonetic, explanation, note, tags, source, created_at, updated_at FROM words",
        );
        let mut conditions: Vec<String> = Vec::new();

        if query.is_some() {
            conditions.push("term LIKE ? OR translation LIKE ?".to_string());
        }

        if tag.is_some() {
            conditions.push("tags LIKE ?".to_string());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");

        let mut db_query = sqlx::query(&sql);
        if let Some(search) = query.as_ref() {
            let like_value = format!("%{search}%");
            db_query = db_query.bind(like_value.clone()).bind(like_value);
        }
        if let Some(tag_value) = tag.as_ref() {
            db_query = db_query.bind(format!("%{tag_value}%"));
        }

        db_query = db_query.bind(limit).bind(offset);

        let rows = db_query
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list words: {e}")))?;

        let mut words = Vec::new();
        for row in rows {
            words.push(self.row_to_word(row)?);
        }

        Ok(words)
    }

    pub async fn get_word(&self, word_id: &UUID) -> Result<Option<Word>, AppError> {
        let query = r#"
            SELECT id, term, language, translation, phonetic, explanation, note, tags, source, created_at, updated_at
            FROM words WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(word_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get word: {e}")))?;

        match row {
            Some(row) => Ok(Some(self.row_to_word(row)?)),
            None => Ok(None),
        }
    }


    pub async fn update_word(&self, word: &Word) -> Result<(), AppError> {
        let tags_json = serde_json::to_string(&word.tags)
            .map_err(|e| AppError::validation_error(&format!("Invalid tags: {e}")))?;

        let query = r#"
            UPDATE words
            SET term = $1, language = $2, translation = $3, phonetic = $4, explanation = $5, note = $6, tags = $7, source = $8, updated_at = $9
            WHERE id = $10
        "#;

        let result = sqlx::query(query)
            .bind(&word.term)
            .bind(&word.language)
            .bind(&word.translation)
            .bind(&word.phonetic)
            .bind(&word.explanation)
            .bind(&word.note)
            .bind(&tags_json)
            .bind(&word.source)
            .bind(word.updated_at)
            .bind(&word.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update word: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Word not found: {}", word.id)));
        }

        Ok(())
    }

    pub async fn delete_word(&self, word_id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM words WHERE id = $1")
            .bind(word_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete word: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Word not found: {}", word_id)));
        }

        Ok(())
    }


    fn row_to_word(&self, row: sqlx::sqlite::SqliteRow) -> Result<Word, AppError> {
        let tags_json: String = row.try_get("tags").unwrap_or_else(|_| "[]".to_string());
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(Word {
            id: row.try_get("id").unwrap_or_default(),
            term: row.try_get("term").unwrap_or_default(),
            language: row.try_get("language").unwrap_or_default(),
            translation: row.try_get("translation").unwrap_or_default(),
            phonetic: row.try_get("phonetic").ok(),
            explanation: row.try_get("explanation").ok(),
            note: row.try_get("note").ok(),
            tags,
            source: row.try_get("source").unwrap_or_default(),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        })
    }
}

