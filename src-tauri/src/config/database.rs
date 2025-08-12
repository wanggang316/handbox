// 数据库配置

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: u64,
    pub idle_timeout: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:handbox.db".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: 30,
            idle_timeout: 600,
        }
    }
}
