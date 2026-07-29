use crate::models::AppError;
use std::path::{Path, PathBuf};

pub struct StorageService {
    data_dir: PathBuf,
}

impl StorageService {
    pub fn new(data_dir: PathBuf) -> Result<Self, AppError> {
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                AppError::internal_error(&format!("Failed to create data directory: {e}"))
            })?;
        }

        Ok(Self { data_dir })
    }

    pub fn get_database_path(&self) -> PathBuf {
        self.data_dir.join("handbox.db")
    }

    pub fn get_config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    pub fn get_mcp_config_path(&self) -> PathBuf {
        self.data_dir.join("mcp.json")
    }

    pub fn get_logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

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

    pub async fn init_database(&self) -> Result<(), AppError> {
        let db_path = self.get_database_path();

        std::fs::create_dir_all(&self.data_dir).map_err(|e| {
            AppError::internal_error(&format!("Failed to create data directory: {e}"))
        })?;

        // Empty placeholder file so the app can start; real initialization is not implemented yet.
        if !db_path.exists() {
            std::fs::write(&db_path, "").map_err(|e| {
                AppError::internal_error(&format!("Failed to create database file: {e}"))
            })?;
        }

        // TODO: full initialization (sqlx connection, schema migrations, indexes)

        Ok(())
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        // TODO: implement database migrations
        Err(AppError::internal_error(
            "Database migrations not implemented yet",
        ))
    }
}
