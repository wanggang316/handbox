// Artifact 数据存储类型定义

use serde::{Deserialize, Serialize};

use super::{Timestamp, UUID};

/// Artifact 类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Shell,
    Python,
    Web,
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactType::Shell => write!(f, "shell"),
            ArtifactType::Python => write!(f, "python"),
            ArtifactType::Web => write!(f, "web"),
        }
    }
}

impl std::str::FromStr for ArtifactType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shell" => Ok(ArtifactType::Shell),
            "python" => Ok(ArtifactType::Python),
            "web" => Ok(ArtifactType::Web),
            _ => Err(format!("Invalid artifact type: {}", s)),
        }
    }
}

/// 模型参数配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub max_tokens: Option<i32>,
}

/// 执行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64, // milliseconds
}

fn default_timeout() -> u64 {
    30000 // 30 seconds
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            args: vec![],
            env: std::collections::HashMap::new(),
            permissions: vec![],
            timeout: default_timeout(),
        }
    }
}

/// Artifact 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: UUID,
    pub name: String,
    pub description: Option<String>,

    // Application type
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,

    // Code/Resource paths
    pub entry_file: String,
    pub source_path: Option<String>,

    // Optional AI model configuration
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model_parameters: Option<ModelParameters>,
    pub tools: Option<Vec<String>>, // MCP server names or tool identifiers

    // Execution configuration
    pub execution_config: ExecutionConfig,

    // Installation & lifecycle
    pub is_builtin: bool,
    pub is_installed: bool,
    pub installed_version: Option<String>,
    pub installed_at: Option<Timestamp>,
    pub last_run_at: Option<Timestamp>,
    pub run_count: i32,

    // Metadata
    pub tags: Vec<String>,
    pub icon: Option<String>,
    pub author: Option<String>,

    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateArtifactRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub entry_file: String,
    pub source_path: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model_parameters: Option<ModelParameters>,
    pub tools: Option<Vec<String>>,
    pub execution_config: Option<ExecutionConfig>,
    pub tags: Option<Vec<String>>,
    pub icon: Option<String>,
}

/// 更新 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateArtifactRequest {
    pub id: UUID,
    pub name: Option<String>,
    pub description: Option<String>,
    pub entry_file: Option<String>,
    pub source_path: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model_parameters: Option<ModelParameters>,
    pub tools: Option<Vec<String>>,
    pub execution_config: Option<ExecutionConfig>,
    pub tags: Option<Vec<String>>,
    pub icon: Option<String>,
}

/// 安装 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct InstallArtifactRequest {
    pub artifact_id: UUID,
    /// 如果 Artifact 需要模型,用户需要选择
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
}

/// 执行 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteArtifactRequest {
    pub artifact_id: UUID,
    /// 运行时参数覆盖
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// 执行结果
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub duration: u64, // milliseconds
    pub error: Option<String>,
}

/// Artifact 列表过滤器
#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactFilter {
    pub search: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub is_builtin: Option<bool>,
    pub is_installed: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
