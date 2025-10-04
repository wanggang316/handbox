// MCP repository - data access for MCP server configurations

use crate::models::{
    AppError, McpConnectionType, McpPrompt, McpResource, McpServer, McpServerStatus, McpTool,
};
use crate::storage::Database;
use sqlx::{sqlite::SqliteRow, Row};
use std::collections::HashMap;
use std::sync::Arc;

/// Repository for persisting MCP servers
#[derive(Clone)]
pub struct McpRepository {
    db: Arc<Database>,
}

impl McpRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// List all MCP servers ordered by creation time
    pub async fn list_servers(&self) -> Result<Vec<McpServer>, AppError> {
        let query = r#"
            SELECT id, name, display_name, description, connection_type, command, args, working_dir, env,
                   endpoint, headers, timeout_ms, enabled, status, tools, prompts, resources, enabled_tools,
                   last_sync_at, last_error, created_at, updated_at
            FROM mcps
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list MCP servers: {}", e)))?;

        rows.into_iter()
            .map(|row| self.row_to_server(row))
            .collect()
    }

    /// Retrieve servers by their identifiers, preserving the provided order
    pub async fn get_servers_by_ids(&self, ids: &[String]) -> Result<Vec<McpServer>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT id, name, display_name, description, connection_type, command, args, working_dir, env, \
                    endpoint, headers, timeout_ms, enabled, status, tools, prompts, resources, enabled_tools, \
                    last_sync_at, last_error, created_at, updated_at \
             FROM mcps WHERE id IN (",
        );

        let mut separated = query_builder.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = query_builder
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to fetch MCP servers: {}", e))
            })?;

        let mut servers: Vec<McpServer> = rows
            .into_iter()
            .map(|row| self.row_to_server(row))
            .collect::<Result<_, _>>()?;

        servers.sort_by_key(|server| {
            ids.iter()
                .position(|id| id == &server.id)
                .unwrap_or(ids.len())
        });

        Ok(servers)
    }

    /// Fetch a server by id
    pub async fn get_server(&self, id: &str) -> Result<Option<McpServer>, AppError> {
        let query = r#"
            SELECT id, name, display_name, description, connection_type, command, args, working_dir, env,
                   endpoint, headers, timeout_ms, enabled, status, tools, prompts, resources, enabled_tools,
                   last_sync_at, last_error, created_at, updated_at
            FROM mcps
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get MCP server: {}", e)))?;

        match row {
            Some(row) => Ok(Some(self.row_to_server(row)?)),
            None => Ok(None),
        }
    }

    /// Create a new server record
    pub async fn create_server(&self, server: &McpServer) -> Result<(), AppError> {
        let query = r#"
            INSERT INTO mcps (
                id, name, display_name, description, connection_type, command, args, working_dir, env,
                endpoint, headers, timeout_ms, enabled, status, tools, prompts, resources, enabled_tools,
                last_sync_at, last_error, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17, $18,
                $19, $20, $21, $22
            )
        "#;

        sqlx::query(query)
            .bind(&server.id)
            .bind(&server.name)
            .bind(&server.display_name)
            .bind(&server.description)
            .bind(server.connection_type.to_string())
            .bind(&server.command)
            .bind(serde_json::to_string(&server.args).unwrap_or_else(|_| "[]".to_string()))
            .bind(&server.working_dir)
            .bind(serde_json::to_string(&server.env).unwrap_or_else(|_| "{}".to_string()))
            .bind(&server.endpoint)
            .bind(serde_json::to_string(&server.headers).unwrap_or_else(|_| "{}".to_string()))
            .bind(server.timeout_ms.map(|t| t as i64))
            .bind(server.enabled)
            .bind(server.status.to_string())
            .bind(serde_json::to_string(&server.tools).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.prompts).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.resources).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.enabled_tools).unwrap_or_else(|_| "[]".to_string()))
            .bind(server.last_sync_at)
            .bind(server.last_error.as_ref().and_then(|e| serde_json::to_string(e).ok()))
            .bind(server.created_at)
            .bind(server.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create MCP server: {}", e))
            })?;

        Ok(())
    }

    /// Update an existing server
    pub async fn update_server(&self, server: &McpServer) -> Result<(), AppError> {
        let query = r#"
            UPDATE mcps SET
                name = $1,
                display_name = $2,
                description = $3,
                connection_type = $4,
                command = $5,
                args = $6,
                working_dir = $7,
                env = $8,
                endpoint = $9,
                headers = $10,
                timeout_ms = $11,
                enabled = $12,
                status = $13,
                tools = $14,
                prompts = $15,
                resources = $16,
                enabled_tools = $17,
                last_sync_at = $18,
                last_error = $19,
                updated_at = $20
            WHERE id = $21
        "#;

        let result = sqlx::query(query)
            .bind(&server.name)
            .bind(&server.display_name)
            .bind(&server.description)
            .bind(server.connection_type.to_string())
            .bind(&server.command)
            .bind(serde_json::to_string(&server.args).unwrap_or_else(|_| "[]".to_string()))
            .bind(&server.working_dir)
            .bind(serde_json::to_string(&server.env).unwrap_or_else(|_| "{}".to_string()))
            .bind(&server.endpoint)
            .bind(serde_json::to_string(&server.headers).unwrap_or_else(|_| "{}".to_string()))
            .bind(server.timeout_ms.map(|t| t as i64))
            .bind(server.enabled)
            .bind(server.status.to_string())
            .bind(serde_json::to_string(&server.tools).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.prompts).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.resources).unwrap_or_else(|_| "[]".to_string()))
            .bind(serde_json::to_string(&server.enabled_tools).unwrap_or_else(|_| "[]".to_string()))
            .bind(server.last_sync_at)
            .bind(server.last_error.as_ref().and_then(|e| serde_json::to_string(e).ok()))
            .bind(server.updated_at)
            .bind(&server.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update MCP server: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "MCP server not found: {}",
                server.id
            )));
        }

        Ok(())
    }

    /// Update enabled flag only
    pub async fn update_enabled(
        &self,
        id: &str,
        enabled: bool,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let query = "UPDATE mcps SET enabled = $1, updated_at = $2 WHERE id = $3";

        let result = sqlx::query(query)
            .bind(enabled)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update MCP server: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "MCP server not found: {}",
                id
            )));
        }

        Ok(())
    }

    /// Update status information
    pub async fn update_status(
        &self,
        id: &str,
        status: McpServerStatus,
        tools: &[McpTool],
        last_sync_at: Option<i64>,
        last_error: Option<String>,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE mcps SET status = $1, tools = $2, last_sync_at = $3, last_error = $4, updated_at = $5
            WHERE id = $6
        "#;

        let result = sqlx::query(query)
            .bind(status.to_string())
            .bind(serde_json::to_string(tools).unwrap_or_else(|_| "[]".to_string()))
            .bind(last_sync_at)
            .bind(last_error)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update MCP server status: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "MCP server not found: {}",
                id
            )));
        }

        Ok(())
    }

    /// Delete a server
    pub async fn delete_server(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM mcps WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete MCP server: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "MCP server not found: {}",
                id
            )));
        }

        Ok(())
    }

    fn row_to_server(&self, row: SqliteRow) -> Result<McpServer, AppError> {
        let args: Vec<String> = row
            .try_get::<Option<String>, _>("args")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let env: HashMap<String, String> = row
            .try_get::<Option<String>, _>("env")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let headers: HashMap<String, String> = row
            .try_get::<Option<String>, _>("headers")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let tools: Vec<McpTool> = row
            .try_get::<Option<String>, _>("tools")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let prompts: Vec<McpPrompt> = row
            .try_get::<Option<String>, _>("prompts")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let resources: Vec<McpResource> = row
            .try_get::<Option<String>, _>("resources")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let enabled_tools: Vec<String> = row
            .try_get::<Option<String>, _>("enabled_tools")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let status_value: String = row.try_get("status")?;
        let connection_type_value: String = row
            .try_get("connection_type")
            .unwrap_or_else(|_| "stdio".to_string());

        Ok(McpServer {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            display_name: row.try_get("display_name").ok(),
            description: row.try_get("description").ok(),
            connection_type: McpConnectionType::from(connection_type_value.as_str()),
            command: row.try_get("command")?,
            args,
            working_dir: row.try_get("working_dir").ok(),
            env,
            endpoint: row.try_get("endpoint").ok(),
            headers,
            timeout_ms: row
                .try_get::<Option<i64>, _>("timeout_ms")?
                .map(|t| t as u64),
            enabled: row.try_get::<i64, _>("enabled")? != 0,
            status: McpServerStatus::from(status_value.as_str()),
            tools,
            prompts,
            resources,
            enabled_tools,
            last_sync_at: row.try_get("last_sync_at").ok(),
            last_error: row
                .try_get::<Option<String>, _>("last_error")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
