// 数据库服务实现

use crate::models::AppError;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Sqlite, SqlitePool};
use std::path::Path;

/// 数据库服务
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// 创建数据库服务实例
    pub async fn new(db_path: &Path) -> Result<Self, AppError> {
        // 确保父目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal_error(&format!("Failed to create database directory: {}", e))
            })?;
        }

        let db_url = format!("sqlite://{}", db_path.display());

        // 如果数据库文件不存在，创建它
        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            match Sqlite::create_database(&db_url).await {
                Ok(()) => tracing::info!("Database created successfully at {}", db_path.display()),
                Err(err) => {
                    return Err(AppError::internal_error(&format!(
                        "Failed to create database: {}",
                        err
                    )))
                }
            }
        }

        // 创建连接池 - 使用单连接避免锁定问题
        let pool = SqlitePoolOptions::new()
            .max_connections(1) // 使用单连接避免锁定
            .min_connections(1)
            .connect(&db_url)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to connect to database: {}", e))
            })?;

        // 配置 SQLite 特定设置
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to set journal mode: {}", e)))?;

        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to set synchronous mode: {}", e))
            })?;

        sqlx::query("PRAGMA busy_timeout = 30000")
            .execute(&pool)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to set busy timeout: {}", e)))?;

        // 运行迁移
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to run migrations: {}", e)))?;

        tracing::info!("Database service initialized successfully");

        Ok(Self { pool })
    }

    /// 获取数据库连接池
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Database health check failed: {}", e))
            })?;

        Ok(())
    }

    /// 获取数据库统计信息
    pub async fn get_stats(&self) -> Result<DatabaseStats, AppError> {
        let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM providers")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get provider count: {}", e))
            })?;

        let model_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get model count: {}", e)))?;

        Ok(DatabaseStats {
            provider_count: provider_count as i32,
            model_count: model_count as i32,
        })
    }
}

/// 数据库统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStats {
    pub provider_count: i32,
    pub model_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_service_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db_service = Database::new(&db_path).await;
        assert!(db_service.is_ok());

        let service = db_service.unwrap();
        assert!(service.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_agent_session_migrations_applied() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_agent_sessions.db");

        let service = Database::new(&db_path).await.unwrap();
        let pool = service.pool();

        // Both new Agent-mode tables exist after migrations 044/045.
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name IN ('agent_sessions', 'agent_session_messages') ORDER BY name",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(
            tables,
            vec![
                "agent_session_messages".to_string(),
                "agent_sessions".to_string()
            ]
        );

        // Their indexes exist as well.
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='index' \
             AND name IN ('idx_agent_sessions_updated_at', 'idx_agent_sessions_name', \
             'idx_agent_session_messages_session_seq') ORDER BY name",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(
            indexes,
            vec![
                "idx_agent_session_messages_session_seq".to_string(),
                "idx_agent_sessions_name".to_string(),
                "idx_agent_sessions_updated_at".to_string()
            ]
        );
    }

    /// Concatenated `quote()` snapshot of every pre-046 agent_sessions column,
    /// used to assert the backfill leaves existing columns byte-identical.
    const AGENT_SESSION_LEGACY_FINGERPRINT: &str = "\
        SELECT quote(id) || '|' || quote(name) || '|' || quote(model_id) || '|' || \
               quote(provider_id) || '|' || quote(system_prompt) || '|' || \
               quote(thinking_level) || '|' || quote(temperature) || '|' || \
               quote(max_tokens) || '|' || quote(working_dir) || '|' || \
               quote(enabled_tools) || '|' || quote(tool_execution_mode) || '|' || \
               quote(message_count) || '|' || quote(last_message_at) || '|' || \
               quote(created_at) || '|' || quote(updated_at) \
        FROM agent_sessions ORDER BY id";

    /// Creates a database with migrations applied only up to `max_version`,
    /// simulating an on-disk database from before the newer migrations existed.
    async fn create_legacy_pool(db_path: &Path, max_version: i64) -> SqlitePool {
        let db_url = format!("sqlite://{}", db_path.display());
        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            Sqlite::create_database(&db_url).await.unwrap();
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        let mut migrator = sqlx::migrate!("./migrations");
        migrator.migrations = std::borrow::Cow::Owned(
            migrator
                .migrations
                .iter()
                .filter(|m| m.version <= max_version)
                .cloned()
                .collect(),
        );
        migrator.run(&pool).await.unwrap();
        pool
    }

    async fn insert_legacy_agent_session(
        pool: &SqlitePool,
        id: &str,
        working_dir: Option<&str>,
        last_message_at: Option<i64>,
        created_at: i64,
    ) {
        sqlx::query(
            "INSERT INTO agent_sessions (
                 id, name, model_id, provider_id, system_prompt, thinking_level,
                 temperature, max_tokens, working_dir, enabled_tools,
                 tool_execution_mode, message_count, last_message_at,
                 created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(format!("session-{id}"))
        .bind("gpt-test")
        .bind("openai")
        .bind("be helpful")
        .bind("medium")
        .bind(0.7_f64)
        .bind(4096_i64)
        .bind(working_dir)
        .bind(r#"["read_file"]"#)
        .bind("auto")
        .bind(2_i64)
        .bind(last_message_at)
        .bind(created_at)
        .bind(created_at)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_agent_projects_migration_applied() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_agent_projects.db");

        let service = Database::new(&db_path).await.unwrap();
        let pool = service.pool();

        // agent_projects table exists after migration 046.
        let table: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='agent_projects'",
        )
        .fetch_optional(pool)
        .await
        .unwrap();
        assert_eq!(table.as_deref(), Some("agent_projects"));

        // agent_sessions gained the project_id column.
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('agent_sessions')")
                .fetch_all(pool)
                .await
                .unwrap();
        assert!(columns.contains(&"project_id".to_string()));

        // UNIQUE(path) is enforced.
        sqlx::query(
            "INSERT INTO agent_projects (id, path, name, created_at, updated_at) \
             VALUES ('p1', '/tmp/unique-check', 'unique-check', 1, 1)",
        )
        .execute(pool)
        .await
        .unwrap();
        let duplicate = sqlx::query(
            "INSERT INTO agent_projects (id, path, name, created_at, updated_at) \
             VALUES ('p2', '/tmp/unique-check', 'unique-check', 2, 2)",
        )
        .execute(pool)
        .await;
        assert!(duplicate.is_err(), "duplicate path must violate UNIQUE");

        // Fresh DB has no sessions, so the backfill created no projects
        // besides the row inserted above.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_projects")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_agent_projects_backfill_from_legacy_sessions() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_agent_projects_backfill.db");

        // Build a pre-046 database (migrations up to 045 only) with legacy rows.
        let seed_pool = create_legacy_pool(&db_path, 45).await;

        // Two sessions share a dir; activity = coalesce(last_message_at, created_at).
        insert_legacy_agent_session(
            &seed_pool,
            "s1",
            Some("/Users/me/dev/alpha"),
            Some(2000),
            1000,
        )
        .await;
        insert_legacy_agent_session(&seed_pool, "s2", Some("/Users/me/dev/alpha"), None, 3000)
            .await;
        insert_legacy_agent_session(&seed_pool, "s3", Some("/Users/me/dev/beta"), Some(500), 400)
            .await;
        // NULL and empty working_dir must stay unlinked.
        insert_legacy_agent_session(&seed_pool, "s4", None, Some(9000), 8000).await;
        insert_legacy_agent_session(&seed_pool, "s5", Some(""), Some(9100), 8100).await;
        // Root path has an empty basename -> project name falls back to the path.
        insert_legacy_agent_session(&seed_pool, "s6", Some("/"), None, 7000).await;

        // Chat-mode and /agents preset rows that migrations 046+ must never touch.
        sqlx::query(
            "INSERT INTO sessions (id, name, message_count, created_at, updated_at) \
             VALUES ('chat1', 'chat', 0, 1, 1)",
        )
        .execute(&seed_pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) \
             VALUES ('msg1', 'chat1', 'user', 'hi', 2, 2)",
        )
        .execute(&seed_pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO agents (id, name, created_at, updated_at) \
             VALUES ('agent1', 'preset', 1, 1)",
        )
        .execute(&seed_pool)
        .await
        .unwrap();

        let fingerprint_before: Vec<String> = sqlx::query_scalar(AGENT_SESSION_LEGACY_FINGERPRINT)
            .fetch_all(&seed_pool)
            .await
            .unwrap();
        assert_eq!(fingerprint_before.len(), 6);
        seed_pool.close().await;

        // Upgrade: Database::new applies 046+ including the backfill.
        let service = Database::new(&db_path).await.unwrap();
        let pool = service.pool();

        // One project per distinct non-empty working_dir.
        let distinct_dirs: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT working_dir) FROM agent_sessions \
             WHERE working_dir IS NOT NULL AND working_dir != ''",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_projects")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(distinct_dirs, 3);
        assert_eq!(project_count, distinct_dirs);

        // Session total unchanged; every non-empty-dir session linked to the
        // project whose path equals its working_dir.
        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(session_count, 6);
        let linked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_sessions s \
             JOIN agent_projects p ON p.id = s.project_id \
             WHERE p.path = s.working_dir",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(linked, 4); // s1, s2, s3, s6
        let unlinked: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM agent_sessions WHERE project_id IS NULL ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(unlinked, vec!["s4".to_string(), "s5".to_string()]);

        // name = basename, falling back to the full path when the basename is
        // empty (root '/'); timestamps = max activity within the directory.
        let projects: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT path, name, created_at, updated_at FROM agent_projects ORDER BY path",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(
            projects,
            vec![
                ("/".to_string(), "/".to_string(), 7000, 7000),
                (
                    "/Users/me/dev/alpha".to_string(),
                    "alpha".to_string(),
                    3000,
                    3000
                ),
                (
                    "/Users/me/dev/beta".to_string(),
                    "beta".to_string(),
                    500,
                    500
                ),
            ]
        );

        // Pre-existing agent_sessions columns are byte-identical.
        let fingerprint_after: Vec<String> = sqlx::query_scalar(AGENT_SESSION_LEGACY_FINGERPRINT)
            .fetch_all(pool)
            .await
            .unwrap();
        assert_eq!(fingerprint_after, fingerprint_before);

        // Chat-mode and preset tables untouched.
        for table in ["sessions", "messages", "agents"] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(count, 1, "table {table} row count changed");
        }

        // Re-running startup migrations is a no-op (sqlx applies each
        // migration exactly once): reopen the database and re-verify.
        let project_ids_before: Vec<String> =
            sqlx::query_scalar("SELECT id FROM agent_projects ORDER BY id")
                .fetch_all(pool)
                .await
                .unwrap();
        pool.close().await;

        let reopened = Database::new(&db_path).await.unwrap();
        let project_ids_after: Vec<String> =
            sqlx::query_scalar("SELECT id FROM agent_projects ORDER BY id")
                .fetch_all(reopened.pool())
                .await
                .unwrap();
        assert_eq!(project_ids_after, project_ids_before);
        let fingerprint_reopened: Vec<String> =
            sqlx::query_scalar(AGENT_SESSION_LEGACY_FINGERPRINT)
                .fetch_all(reopened.pool())
                .await
                .unwrap();
        assert_eq!(fingerprint_reopened, fingerprint_before);
    }

    #[tokio::test]
    async fn test_database_stats() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_stats.db");

        let service = Database::new(&db_path).await.unwrap();
        let stats = service.get_stats().await.unwrap();

        // After migration, we have 0 providers (no predefined data)
        assert_eq!(stats.provider_count, 0);
        assert_eq!(stats.model_count, 0);
    }
}
