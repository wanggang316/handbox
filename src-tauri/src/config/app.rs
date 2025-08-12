// 应用配置

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub security: SecurityConfig,
    pub performance: PerformanceConfig,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub connection_pool_size: u32,
    pub query_timeout_ms: u64,
    pub enable_wal: bool,
    pub enable_fts: bool,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub enable_console: bool,
    pub enable_file: bool,
    pub log_dir: Option<PathBuf>,
    pub max_file_size_mb: u64,
    pub max_files: u32,
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub use_keychain: bool,
    pub encrypt_local_data: bool,
    pub api_key_prefix_length: usize,
}

/// 性能配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_requests: usize,
    pub request_timeout_ms: u64,
    pub cache_size_mb: u64,
    pub gc_interval_seconds: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                path: PathBuf::from("handbox.db"),
                connection_pool_size: 10,
                query_timeout_ms: 30000,
                enable_wal: true,
                enable_fts: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                enable_console: true,
                enable_file: true,
                log_dir: None,
                max_file_size_mb: 10,
                max_files: 5,
            },
            security: SecurityConfig {
                use_keychain: true,
                encrypt_local_data: true,
                api_key_prefix_length: 8,
            },
            performance: PerformanceConfig {
                max_concurrent_requests: 10,
                request_timeout_ms: 120000,
                cache_size_mb: 256,
                gc_interval_seconds: 300,
            },
        }
    }
}

impl AppConfig {
    /// 从文件加载配置
    pub fn load_from_file(
        path: &PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !path.exists() {
            // 如果配置文件不存在，创建默认配置
            let config = Self::default();
            config.save_to_file(path)?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file(
        &self,
        path: &PathBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 确保目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
