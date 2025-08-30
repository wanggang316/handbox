// 数据库服务实现

use crate::models::AppError;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Sqlite, SqlitePool};
use std::path::Path;

/// 数据库服务
#[derive(Clone)]
pub struct DatabaseService {
    pool: SqlitePool,
}

impl DatabaseService {
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
            .max_connections(1)  // 使用单连接避免锁定
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
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to set journal mode: {}", e))
            })?;
            
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to set synchronous mode: {}", e))
            })?;
            
        sqlx::query("PRAGMA busy_timeout = 30000")
            .execute(&pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to set busy timeout: {}", e))
            })?;

        // 运行迁移
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to run migrations: {}", e))
            })?;

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
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get model count: {}", e))
            })?;

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
        
        let db_service = DatabaseService::new(&db_path).await;
        assert!(db_service.is_ok());
        
        let service = db_service.unwrap();
        assert!(service.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_database_stats() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_stats.db");
        
        let service = DatabaseService::new(&db_path).await.unwrap();
        let stats = service.get_stats().await.unwrap();
        
        // After migration, we have 5 predefined providers
        assert_eq!(stats.provider_count, 5);
        assert_eq!(stats.model_count, 0);
    }
}