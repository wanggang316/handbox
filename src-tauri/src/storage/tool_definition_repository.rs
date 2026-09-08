// Data access for user-defined agent tools (`tool_definitions`, migration 069).
//
// [`ToolDefinitionRepository::list_enabled`] runs once per agent run to assemble
// the tool set, and is served by `idx_tool_definitions_enabled`; everything else
// here serves the settings UI.
//
// `parameters` is stored as a JSON array in one TEXT column. A row whose JSON is
// unreadable decodes to an EMPTY parameter list rather than failing the read:
// losing one tool's arguments is recoverable in the editor, whereas a hard error
// would take the whole settings page — and the run — down with it.

use crate::models::AppError;
use crate::storage::types::{
    CreateToolDefinitionRequest, ToolDefinition, ToolParam, UpdateToolDefinitionRequest,
};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Every column, in a fixed order shared by all reads.
const TOOL_COLUMNS: &str = r#"
    SELECT id, name, display_name, icon, description, parameters,
           genui_id, enabled, sort_order, created_at, updated_at
    FROM tool_definitions
"#;

/// Empty string means "clear the column"; see [`UpdateToolDefinitionRequest`].
fn blank_to_null(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

fn encode_parameters(parameters: &[ToolParam]) -> String {
    serde_json::to_string(parameters).unwrap_or_else(|_| "[]".to_string())
}

fn decode_parameters(raw: &str, id: &str) -> Vec<ToolParam> {
    match serde_json::from_str(raw) {
        Ok(parameters) => parameters,
        Err(e) => {
            tracing::warn!(
                tool = id,
                "unreadable tool parameters, treating as empty: {}",
                e
            );
            Vec::new()
        }
    }
}

pub struct ToolDefinitionRepository {
    db: Arc<Database>,
}

impl ToolDefinitionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_definition(row: sqlx::sqlite::SqliteRow) -> Result<ToolDefinition, AppError> {
        let id: String = row.try_get("id").map_err(decode_err)?;
        let parameters: String = row.try_get("parameters").map_err(decode_err)?;
        Ok(ToolDefinition {
            parameters: decode_parameters(&parameters, &id),
            id,
            name: row.try_get("name").map_err(decode_err)?,
            display_name: row.try_get("display_name").map_err(decode_err)?,
            icon: row.try_get("icon").map_err(decode_err)?,
            description: row.try_get("description").map_err(decode_err)?,
            genui_id: row.try_get("genui_id").map_err(decode_err)?,
            enabled: row.try_get::<i64, _>("enabled").map_err(decode_err)? != 0,
            sort_order: row.try_get("sort_order").map_err(decode_err)?,
            created_at: row.try_get("created_at").map_err(decode_err)?,
            updated_at: row.try_get("updated_at").map_err(decode_err)?,
        })
    }

    /// Enabled definitions in display order — the snapshot a run registers.
    pub async fn list_enabled(&self) -> Result<Vec<ToolDefinition>, AppError> {
        let query =
            format!("{TOOL_COLUMNS} WHERE enabled = 1 ORDER BY sort_order ASC, created_at ASC");
        let rows = sqlx::query(sqlx::AssertSqlSafe(query))
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list tool definitions: {}", e))
            })?;

        rows.into_iter().map(Self::row_to_definition).collect()
    }

    /// All definitions, enabled or not, for the settings UI.
    pub async fn list(&self) -> Result<Vec<ToolDefinition>, AppError> {
        let query = format!("{TOOL_COLUMNS} ORDER BY sort_order ASC, created_at ASC");
        let rows = sqlx::query(sqlx::AssertSqlSafe(query))
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list tool definitions: {}", e))
            })?;

        rows.into_iter().map(Self::row_to_definition).collect()
    }

    pub async fn get(&self, id: &str) -> Result<Option<ToolDefinition>, AppError> {
        let query = format!("{TOOL_COLUMNS} WHERE id = $1");
        let row = sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get tool definition: {}", e))
            })?;

        row.map(Self::row_to_definition).transpose()
    }

    /// Lookup by registration name — how the service layer enforces uniqueness
    /// before the UNIQUE index turns it into an opaque SQL error.
    pub async fn get_by_name(&self, name: &str) -> Result<Option<ToolDefinition>, AppError> {
        let query = format!("{TOOL_COLUMNS} WHERE name = $1");
        let row = sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(name)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get tool definition: {}", e))
            })?;

        row.map(Self::row_to_definition).transpose()
    }

    /// Insert a definition. An omitted `sort_order` appends to the end, so a
    /// tool created from the UI lands last in the list.
    pub async fn create(
        &self,
        request: CreateToolDefinitionRequest,
        now: i64,
    ) -> Result<ToolDefinition, AppError> {
        let sort_order = match request.sort_order {
            Some(order) => order,
            None => self.next_sort_order().await?,
        };

        let definition = ToolDefinition {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            display_name: request.display_name,
            icon: blank_to_null(request.icon),
            description: request.description,
            parameters: request.parameters,
            genui_id: blank_to_null(request.genui_id),
            enabled: true,
            sort_order,
            created_at: now,
            updated_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO tool_definitions
                (id, name, display_name, icon, description, parameters,
                 genui_id, enabled, sort_order, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
        )
        .bind(&definition.id)
        .bind(&definition.name)
        .bind(&definition.display_name)
        .bind(&definition.icon)
        .bind(&definition.description)
        .bind(encode_parameters(&definition.parameters))
        .bind(&definition.genui_id)
        .bind(i64::from(definition.enabled))
        .bind(definition.sort_order)
        .bind(definition.created_at)
        .bind(definition.updated_at)
        .execute(self.db.pool())
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to create tool definition: {}", e))
        })?;

        Ok(definition)
    }

    /// Read-modify-write: an omitted field keeps the stored value. Done in Rust
    /// rather than a dynamic SET clause because the update is not hot and the
    /// column list is small.
    pub async fn update(
        &self,
        id: &str,
        request: UpdateToolDefinitionRequest,
        now: i64,
    ) -> Result<ToolDefinition, AppError> {
        let Some(current) = self.get(id).await? else {
            return Err(AppError::not_found(&format!(
                "Tool definition not found: {}",
                id
            )));
        };

        let updated = ToolDefinition {
            name: request.name.unwrap_or(current.name),
            display_name: request.display_name.unwrap_or(current.display_name),
            icon: request
                .icon
                .map_or(current.icon, |v| blank_to_null(Some(v))),
            description: request.description.unwrap_or(current.description),
            parameters: request.parameters.unwrap_or(current.parameters),
            genui_id: request
                .genui_id
                .map_or(current.genui_id, |v| blank_to_null(Some(v))),
            enabled: request.enabled.unwrap_or(current.enabled),
            sort_order: request.sort_order.unwrap_or(current.sort_order),
            updated_at: now,
            ..current
        };

        sqlx::query(
            r#"
            UPDATE tool_definitions SET
                name = $1, display_name = $2, icon = $3, description = $4,
                parameters = $5, genui_id = $6, enabled = $7, sort_order = $8,
                updated_at = $9
            WHERE id = $10
        "#,
        )
        .bind(&updated.name)
        .bind(&updated.display_name)
        .bind(&updated.icon)
        .bind(&updated.description)
        .bind(encode_parameters(&updated.parameters))
        .bind(&updated.genui_id)
        .bind(i64::from(updated.enabled))
        .bind(updated.sort_order)
        .bind(updated.updated_at)
        .bind(id)
        .execute(self.db.pool())
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to update tool definition: {}", e))
        })?;

        Ok(updated)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM tool_definitions WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete tool definition: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Tool definition not found: {}",
                id
            )));
        }

        Ok(())
    }

    /// One past the highest stored order, so a new tool lands last.
    async fn next_sort_order(&self) -> Result<i64, AppError> {
        let max: Option<i64> = sqlx::query_scalar("SELECT MAX(sort_order) FROM tool_definitions")
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to read sort order: {}", e)))?;

        Ok(max.map_or(0, |m| m + 1))
    }
}

fn decode_err(e: sqlx::Error) -> AppError {
    AppError::internal_error(&format!("Failed to decode tool definition row: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::ToolParamType;
    use crate::storage::Database;
    use tempfile::TempDir;

    async fn repo() -> (ToolDefinitionRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(
            Database::new(&dir.path().join("test.db"))
                .await
                .expect("database"),
        );
        (ToolDefinitionRepository::new(db), dir)
    }

    fn create_request(name: &str) -> CreateToolDefinitionRequest {
        CreateToolDefinitionRequest {
            name: name.to_string(),
            display_name: "Weather card".to_string(),
            icon: Some("cloud-sun".to_string()),
            description: "Call when the user asks about the weather".to_string(),
            parameters: vec![
                ToolParam {
                    name: "city".to_string(),
                    param_type: ToolParamType::String,
                    description: "City name".to_string(),
                    required: true,
                },
                ToolParam {
                    name: "temp".to_string(),
                    param_type: ToolParamType::Number,
                    description: "Temperature in celsius".to_string(),
                    required: false,
                },
            ],
            genui_id: Some("genui-1".to_string()),
            sort_order: None,
        }
    }

    #[tokio::test]
    async fn create_then_get_roundtrips_every_field() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(create_request("weather_card"), 1_700_000_000_000)
            .await
            .unwrap();

        let fetched = repo.get(&created.id).await.unwrap().expect("stored tool");
        assert_eq!(fetched, created);
        assert_eq!(fetched.parameters.len(), 2, "parameters survive the column");
        assert_eq!(fetched.parameters[0].name, "city");
        assert!(fetched.parameters[0].required);
        assert_eq!(fetched.parameters[1].param_type, ToolParamType::Number);
        assert!(fetched.enabled, "a new tool is enabled");
    }

    /// Every parameter type survives the JSON column — the failure this guards
    /// is adding a variant whose serde tag does not round-trip.
    #[tokio::test]
    async fn every_parameter_type_roundtrips() {
        let (repo, _dir) = repo().await;
        let types = [
            ToolParamType::String,
            ToolParamType::Number,
            ToolParamType::Boolean,
            ToolParamType::StringList,
            ToolParamType::StringMatrix,
            ToolParamType::KeyValueList,
        ];
        let created = repo
            .create(
                CreateToolDefinitionRequest {
                    parameters: types
                        .iter()
                        .enumerate()
                        .map(|(i, t)| ToolParam {
                            name: format!("p{i}"),
                            param_type: *t,
                            description: String::new(),
                            required: false,
                        })
                        .collect(),
                    ..create_request("all_types")
                },
                1,
            )
            .await
            .unwrap();

        let fetched = repo.get(&created.id).await.unwrap().expect("stored tool");
        let stored: Vec<ToolParamType> = fetched.parameters.iter().map(|p| p.param_type).collect();
        assert_eq!(stored, types);
    }

    /// Losing one tool's arguments must not take the settings page (or a run)
    /// down with it.
    #[tokio::test]
    async fn a_corrupt_parameters_column_decodes_as_empty() {
        let (repo, _dir) = repo().await;
        let created = repo.create(create_request("broken"), 1).await.unwrap();
        sqlx::query("UPDATE tool_definitions SET parameters = 'not json' WHERE id = $1")
            .bind(&created.id)
            .execute(repo.db.pool())
            .await
            .unwrap();

        let fetched = repo
            .get(&created.id)
            .await
            .unwrap()
            .expect("row still reads");
        assert!(fetched.parameters.is_empty());
    }

    #[tokio::test]
    async fn get_by_name_finds_the_registration_name() {
        let (repo, _dir) = repo().await;
        repo.create(create_request("weather_card"), 1)
            .await
            .unwrap();

        assert!(repo.get_by_name("weather_card").await.unwrap().is_some());
        assert!(repo.get_by_name("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_enabled_filters_and_orders() {
        let (repo, _dir) = repo().await;
        for (name, order) in [("third", 30), ("first", 10), ("second", 20)] {
            repo.create(
                CreateToolDefinitionRequest {
                    sort_order: Some(order),
                    ..create_request(name)
                },
                1,
            )
            .await
            .unwrap();
        }
        let off = repo
            .create(
                CreateToolDefinitionRequest {
                    sort_order: Some(5),
                    ..create_request("off")
                },
                1,
            )
            .await
            .unwrap();
        repo.update(
            &off.id,
            UpdateToolDefinitionRequest {
                enabled: Some(false),
                ..Default::default()
            },
            2,
        )
        .await
        .unwrap();

        let names: Vec<String> = repo
            .list_enabled()
            .await
            .unwrap()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(names, vec!["first", "second", "third"]);
        assert_eq!(
            repo.list().await.unwrap().len(),
            4,
            "list keeps the disabled"
        );
    }

    /// A tool created without an explicit order appends, so adding one from the
    /// UI never reorders the existing list.
    #[tokio::test]
    async fn omitted_sort_order_appends() {
        let (repo, _dir) = repo().await;
        repo.create(
            CreateToolDefinitionRequest {
                sort_order: Some(5),
                ..create_request("existing")
            },
            1,
        )
        .await
        .unwrap();

        let appended = repo.create(create_request("new_tool"), 2).await.unwrap();
        assert_eq!(appended.sort_order, 6);
    }

    #[tokio::test]
    async fn update_leaves_omitted_fields_untouched() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(create_request("weather_card"), 1)
            .await
            .unwrap();

        let updated = repo
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    display_name: Some("Weather".to_string()),
                    ..Default::default()
                },
                2,
            )
            .await
            .unwrap();

        assert_eq!(updated.display_name, "Weather");
        assert_eq!(updated.name, created.name);
        assert_eq!(updated.parameters, created.parameters);
        assert_eq!(updated.genui_id, created.genui_id);
        assert_eq!(
            updated.created_at, created.created_at,
            "create time is fixed"
        );
        assert_eq!(updated.updated_at, 2);
    }

    /// The documented escape hatch for a flat `Option`: empty string clears —
    /// how the UI unbinds a GenUI without a separate command.
    #[tokio::test]
    async fn an_empty_string_clears_a_nullable_column() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(create_request("weather_card"), 1)
            .await
            .unwrap();
        assert!(created.genui_id.is_some(), "precondition");

        let updated = repo
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    genui_id: Some(String::new()),
                    icon: Some(String::new()),
                    ..Default::default()
                },
                2,
            )
            .await
            .unwrap();
        assert_eq!(updated.genui_id, None);
        assert_eq!(updated.icon, None);
    }

    /// Clearing the list is a real edit, not "leave alone" — `Some(vec![])`
    /// must reach the column.
    #[tokio::test]
    async fn an_empty_parameter_list_is_written() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(create_request("weather_card"), 1)
            .await
            .unwrap();

        let updated = repo
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    parameters: Some(Vec::new()),
                    ..Default::default()
                },
                2,
            )
            .await
            .unwrap();
        assert!(updated.parameters.is_empty());
        assert!(repo
            .get(&created.id)
            .await
            .unwrap()
            .unwrap()
            .parameters
            .is_empty());
    }

    #[tokio::test]
    async fn delete_removes_the_tool_and_reports_a_missing_one() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(create_request("weather_card"), 1)
            .await
            .unwrap();

        repo.delete(&created.id).await.unwrap();
        assert!(repo.get(&created.id).await.unwrap().is_none());
        assert!(
            repo.delete(&created.id).await.is_err(),
            "deleting a missing tool is an error, not a silent no-op"
        );
    }
}
