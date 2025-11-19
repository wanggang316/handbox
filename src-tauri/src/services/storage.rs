// 存储服务实现

use crate::models::AppError;
use std::path::{Path, PathBuf};

/// 存储服务
pub struct StorageService {
    data_dir: PathBuf,
}

impl StorageService {
    pub fn new(data_dir: PathBuf) -> Result<Self, AppError> {
        // 确保数据目录存在
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                AppError::internal_error(&format!("Failed to create data directory: {e}"))
            })?;
        }

        Ok(Self { data_dir })
    }

    /// 获取数据库路径
    pub fn get_database_path(&self) -> PathBuf {
        self.data_dir.join("handbox.db")
    }

    /// 获取配置文件路径
    pub fn get_config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    /// 获取 MCP 配置路径
    pub fn get_mcp_config_path(&self) -> PathBuf {
        self.data_dir.join("mcp.json")
    }

    /// 获取搜索历史存储路径
    pub fn get_search_history_path(&self) -> PathBuf {
        self.data_dir.join("search_history.json")
    }

    /// 获取日志目录
    pub fn get_logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// 确保消息的媒体目录存在（按 chat/message 组织）
    pub fn prepare_message_media_dir(
        &self,
        chat_id: &str,
        message_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self
            .data_dir
            .join("generated_media")
            .join(chat_id)
            .join(message_id);
        Self::ensure_dir(&dir)?;
        Ok(dir)
    }

    /// 确保消息的附件目录存在（用于输入资源）
    pub fn prepare_message_attachment_dir(
        &self,
        chat_id: &str,
        message_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self
            .data_dir
            .join("message_attachments")
            .join(chat_id)
            .join(message_id);
        Self::ensure_dir(&dir)?;
        Ok(dir)
    }

    fn ensure_dir(path: &Path) -> Result<(), AppError> {
        if path.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(path).map_err(|e| {
            AppError::internal_error(&format!("Failed to create storage directory: {e}"))
        })
    }

    /// 初始化数据库
    pub async fn init_database(&self) -> Result<(), AppError> {
        let db_path = self.get_database_path();

        // 确保数据目录存在
        std::fs::create_dir_all(&self.data_dir).map_err(|e| {
            AppError::internal_error(&format!("Failed to create data directory: {e}"))
        })?;

        // 创建一个空的数据库文件以确保应用程序可以启动
        // 这是一个临时解决方案，实际的数据库初始化将在后续开发中完成
        if !db_path.exists() {
            std::fs::write(&db_path, "").map_err(|e| {
                AppError::internal_error(&format!("Failed to create database file: {e}"))
            })?;
        }

        // TODO: 实现完整的数据库初始化
        // - 使用 sqlx 连接到 SQLite 数据库
        // - 运行迁移脚本创建表结构
        // - 创建必要的索引

        Ok(())
    }

    /// 运行数据库迁移
    pub async fn run_migrations(&self) -> Result<(), AppError> {
        // TODO: 实现数据库迁移
        Err(AppError::internal_error(
            "Database migrations not implemented yet",
        ))
    }
}
