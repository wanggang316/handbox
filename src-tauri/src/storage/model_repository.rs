// Model 数据访问层

use crate::models::AppError;
use crate::storage::types::Model;
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Model 仓储层
#[derive(Clone)]
pub struct ModelRepository {
    db: Arc<Database>,
}

impl ModelRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建模型
    pub async fn create_model(&self, model: &Model) -> Result<(), AppError> {
        let features_json = model.features_to_json();

        let input_modalities_json = Model::modalities_to_json(&model.input_modalities);
        let output_modalities_json = Model::modalities_to_json(&model.output_modalities);
        let metadata_json = model.metadata_to_json();
        let pricing_json = model.pricing_to_json();
        let support_parameters_json = model.support_parameters_to_json();
        let default_parameters_json = model.default_parameters_to_json();
        let max_parameters_json = model.max_parameters_to_json();
        let supported_methods_json = model.supported_methods_to_json();

        let query = r#"
            INSERT INTO models (
                id,
                provider_id,
                name,
                description,
                context_length,
                output_max_tokens,
                pricing,
                input_modalities,
                output_modalities,
                support_parameters,
                default_parameters,
                max_parameters,
                supported_features,
                supported_methods,
                metadata,
                url,
                model_created_at,
                enabled,
                favorite,
                created_at,
                updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
            )
        "#;

        sqlx::query(query)
            .bind(&model.id)
            .bind(&model.provider_id)
            .bind(&model.name)
            .bind(model.description.as_deref())
            .bind(model.context_length)
            .bind(model.output_max_tokens)
            .bind(pricing_json.as_deref())
            .bind(input_modalities_json.as_deref())
            .bind(output_modalities_json.as_deref())
            .bind(support_parameters_json.as_deref())
            .bind(default_parameters_json.as_deref())
            .bind(max_parameters_json.as_deref())
            .bind(&features_json)
            .bind(supported_methods_json.as_deref())
            .bind(metadata_json.as_deref())
            .bind(model.url.as_deref())
            .bind(model.model_created_at)
            .bind(model.enabled)
            .bind(model.favorite)
            .bind(model.created_at)
            .bind(model.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create model: {}", e)))?;

        Ok(())
    }

    /// 同步供应商的模型列表（保留用户设置的状态）
    pub async fn sync_provider_models(
        &self,
        provider_id: &str,
        new_models: &[Model],
    ) -> Result<(), AppError> {
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to start transaction: {}", e))
        })?;

        // 1. 获取现有模型的用户状态（enabled, favorite）
        let existing_states =
            sqlx::query(r#"SELECT id, enabled, favorite FROM models WHERE provider_id = $1"#)
                .bind(provider_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to get existing model states: {}", e))
                })?;

        // 构建状态映射表（model_id -> (enabled, favorite)）
        let mut state_map: std::collections::HashMap<String, (bool, bool)> =
            std::collections::HashMap::new();
        for row in existing_states {
            let id: String = row.get("id");
            let enabled: bool = row.get("enabled");
            let favorite: bool = row.get("favorite");
            tracing::debug!(
                "Found existing model state: id={}, enabled={}, favorite={}",
                id,
                enabled,
                favorite
            );
            state_map.insert(id, (enabled, favorite));
        }
        tracing::info!("Built state map for {} existing models", state_map.len());

        // 2. 删除该供应商的所有现有模型
        sqlx::query(r#"DELETE FROM models WHERE provider_id = $1"#)
            .bind(provider_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete existing models: {}", e))
            })?;

        // 3. 插入新模型，保留用户状态
        tracing::info!("Inserting {} new models", new_models.len());
        for model in new_models {
            let features_json = model.features_to_json();
            let input_modalities_json = Model::modalities_to_json(&model.input_modalities);
            let output_modalities_json = Model::modalities_to_json(&model.output_modalities);
            let metadata_json = model.metadata_to_json();
            let pricing_json = model.pricing_to_json();
            let support_parameters_json = model.support_parameters_to_json();
            let default_parameters_json = model.default_parameters_to_json();
            let max_parameters_json = model.max_parameters_to_json();
            let supported_methods_json = model.supported_methods_to_json();

            // 从状态映射中获取用户设置的状态，如果没有则使用默认值
            let (enabled, favorite) = match state_map.get(&model.id) {
                Some((e, f)) => {
                    tracing::debug!(
                        "Preserving state for model {}: enabled={}, favorite={}",
                        model.id,
                        e,
                        f
                    );
                    (*e, *f)
                }
                None => {
                    tracing::debug!("No existing state for model {}, using defaults: enabled=true, favorite=false", model.id);
                    (true, false)
                }
            };

            let query = r#"
                INSERT INTO models (
                    id,
                    provider_id,
                    name,
                    description,
                    context_length,
                    output_max_tokens,
                    pricing,
                    input_modalities,
                    output_modalities,
                    support_parameters,
                    default_parameters,
                    max_parameters,
                    supported_features,
                    supported_methods,
                    metadata,
                    url,
                    model_created_at,
                    enabled,
                    favorite,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
                )
            "#;

            sqlx::query(query)
                .bind(&model.id)
                .bind(&model.provider_id)
                .bind(&model.name)
                .bind(model.description.as_deref())
                .bind(model.context_length)
                .bind(model.output_max_tokens)
                .bind(pricing_json.as_deref())
                .bind(input_modalities_json.as_deref())
                .bind(output_modalities_json.as_deref())
                .bind(support_parameters_json.as_deref())
                .bind(default_parameters_json.as_deref())
                .bind(max_parameters_json.as_deref())
                .bind(&features_json)
                .bind(supported_methods_json.as_deref())
                .bind(metadata_json.as_deref())
                .bind(model.url.as_deref())
                .bind(model.model_created_at)
                .bind(enabled)
                .bind(favorite)
                .bind(model.created_at)
                .bind(model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!(
                        "Failed to insert model {}: {}",
                        model.name, e
                    ))
                })?;
        }

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// 批量创建模型
    pub async fn create_models(&self, models: &[Model]) -> Result<(), AppError> {
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to start transaction: {}", e))
        })?;

        for model in models {
            let features_json = model.features_to_json();
            let input_modalities_json = Model::modalities_to_json(&model.input_modalities);
            let output_modalities_json = Model::modalities_to_json(&model.output_modalities);
            let metadata_json = model.metadata_to_json();
            let pricing_json = model.pricing_to_json();
            let support_parameters_json = model.support_parameters_to_json();
            let default_parameters_json = model.default_parameters_to_json();
            let max_parameters_json = model.max_parameters_to_json();
            let supported_methods_json = model.supported_methods_to_json();

            let query = r#"
                INSERT OR REPLACE INTO models (
                    id,
                    provider_id,
                    name,
                    context_length,
                    output_max_tokens,
                    supported_features,
                    description,
                    input_modalities,
                    output_modalities,
                    metadata,
                    pricing,
                    support_parameters,
                    default_parameters,
                    max_parameters,
                    supported_methods,
                    url,
                    model_created_at,
                    enabled,
                    favorite,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
                )
            "#;

            sqlx::query(query)
                .bind(&model.id)
                .bind(&model.provider_id)
                .bind(&model.name)
                .bind(model.context_length)
                .bind(model.output_max_tokens)
                .bind(features_json)
                .bind(model.description.as_deref())
                .bind(input_modalities_json.as_deref())
                .bind(output_modalities_json.as_deref())
                .bind(metadata_json.as_deref())
                .bind(pricing_json.as_deref())
                .bind(support_parameters_json.as_deref())
                .bind(default_parameters_json.as_deref())
                .bind(max_parameters_json.as_deref())
                .bind(supported_methods_json.as_deref())
                .bind(model.url.as_deref())
                .bind(model.model_created_at)
                .bind(model.enabled)
                .bind(model.favorite)
                .bind(model.created_at)
                .bind(model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to create model {}: {}", model.id, e))
                })?;
        }

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// 获取供应商的所有模型
    pub async fn get_models_by_provider(&self, provider_id: &str) -> Result<Vec<Model>, AppError> {
        let query = r#"
            SELECT
                id,
                provider_id,
                name,
                description,
                context_length,
                output_max_tokens,
                pricing,
                input_modalities,
                output_modalities,
                support_parameters,
                default_parameters,
                max_parameters,
                supported_features,
                supported_methods,
                metadata,
                url,
                model_created_at,
                enabled,
                favorite,
                created_at,
                updated_at
            FROM models
            WHERE provider_id = $1
            ORDER BY
                CASE
                    WHEN model_created_at IS NOT NULL THEN model_created_at
                    ELSE created_at
                END DESC
        "#;

        let rows = sqlx::query(query)
            .bind(provider_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get models: {}", e)))?;

        let mut models = Vec::new();
        for row in rows {
            models.push(self.row_to_model(row)?);
        }

        Ok(models)
    }

    /// 更新模型启用状态
    pub async fn toggle_model(
        &self,
        provider_id: &str,
        model_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            "UPDATE models SET enabled = $1, updated_at = $2 WHERE provider_id = $3 AND id = $4",
        )
        .bind(enabled)
        .bind(now)
        .bind(provider_id)
        .bind(model_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to toggle model: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Model not found"));
        }

        Ok(())
    }

    /// 更新模型收藏状态
    pub async fn toggle_favorite_model(
        &self,
        provider_id: &str,
        model_id: &str,
        favorite: bool,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            "UPDATE models SET favorite = $1, updated_at = $2 WHERE provider_id = $3 AND id = $4",
        )
        .bind(favorite)
        .bind(now)
        .bind(provider_id)
        .bind(model_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to toggle model favorite: {}", e))
        })?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Model not found"));
        }

        Ok(())
    }

    /// 删除供应商的所有模型
    pub async fn delete_models_by_provider(&self, provider_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM models WHERE provider_id = $1")
            .bind(provider_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete models: {}", e)))?;

        Ok(())
    }

    // 辅助方法：将数据库行转换为 Model
    fn row_to_model(&self, row: sqlx::sqlite::SqliteRow) -> Result<Model, AppError> {
        let features_json: String = row.try_get("supported_features")?;
        let supported_features = Model::features_from_json(&features_json).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse model features: {}", e))
        })?;

        let description: Option<String> = row.try_get("description")?;
        let input_modalities: Option<String> = row.try_get("input_modalities")?;
        let output_modalities: Option<String> = row.try_get("output_modalities")?;
        let metadata_raw: Option<String> = row.try_get("metadata")?;
        let pricing_raw: Option<String> = row.try_get("pricing")?;
        let support_parameters_raw: Option<String> = row.try_get("support_parameters").ok();
        let default_parameters_raw: Option<String> = row.try_get("default_parameters").ok();
        let max_parameters_raw: Option<String> = row.try_get("max_parameters").ok();
        let url: Option<String> = row.try_get("url")?;

        let input_modalities =
            Model::modalities_from_json(input_modalities.as_deref()).map_err(|e| {
                AppError::internal_error(&format!("Failed to parse model input modalities: {}", e))
            })?;

        let output_modalities =
            Model::modalities_from_json(output_modalities.as_deref()).map_err(|e| {
                AppError::internal_error(&format!("Failed to parse model output modalities: {}", e))
            })?;

        let metadata = Model::metadata_from_json(metadata_raw.as_deref()).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse model metadata: {}", e))
        })?;

        let pricing = Model::pricing_from_json(pricing_raw.as_deref()).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse model pricing: {}", e))
        })?;

        let support_parameters = Model::support_parameters_from_json(
            support_parameters_raw.as_deref(),
        )
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to parse model support parameters: {}", e))
        })?;

        let default_parameters = Model::default_parameters_from_json(
            default_parameters_raw.as_deref(),
        )
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to parse model default parameters: {}", e))
        })?;

        let max_parameters = Model::max_parameters_from_json(max_parameters_raw.as_deref())
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to parse model max parameters: {}", e))
            })?;

        let supported_methods_raw: Option<String> = row.try_get("supported_methods").ok();
        let supported_methods =
            Model::supported_methods_from_json(supported_methods_raw.as_deref()).map_err(|e| {
                AppError::internal_error(&format!("Failed to parse model supported methods: {}", e))
            })?;

        Ok(Model {
            id: row.try_get("id")?,
            provider_id: row.try_get("provider_id")?,
            name: row.try_get("name")?,
            context_length: row.try_get("context_length")?,
            output_max_tokens: row.try_get("output_max_tokens")?,
            supported_features,
            description,
            input_modalities,
            output_modalities,
            metadata,
            pricing,
            url,
            support_parameters,
            default_parameters,
            max_parameters,
            supported_methods,
            model_created_at: row.try_get("model_created_at").ok(),
            enabled: row.try_get("enabled")?,
            favorite: row.try_get("favorite").unwrap_or(false),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
