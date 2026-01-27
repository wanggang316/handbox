// Agent 相关 IPC 命令

use crate::models::AppError;
use crate::services::{AgentParameter, AgentService};
use crate::storage::types::{Agent, AgentReasoningConfig, UUID};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentCreateRequest {
    pub name: String,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub streaming: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<crate::storage::types::McpServerConfig>>,
    pub skills: Option<Vec<String>>,
}

/// 创建新的 Agent
#[tauri::command]
pub async fn agent_create(
    request: AgentCreateRequest,
    agent_service: State<'_, AgentService>,
) -> Result<Agent, AppError> {
    agent_service
        .create_agent(
            request.name,
            request.model_id,
            request.provider_id,
            request.temperature,
            request.top_p,
            request.top_k,
            request.reasoning,
            request.max_tokens,
            request.streaming,
            request.system_prompt,
            request.mcp_servers,
            request.skills,
        )
        .await
}

/// 获取 Agent 列表
#[tauri::command]
pub async fn agent_list(
    limit: Option<i32>,
    offset: Option<i32>,
    agent_service: State<'_, AgentService>,
) -> Result<Vec<Agent>, AppError> {
    agent_service.list_agents(limit, offset).await
}

/// 获取 Agent 详情
#[tauri::command]
pub async fn agent_get(
    agent_id: UUID,
    agent_service: State<'_, AgentService>,
) -> Result<Agent, AppError> {
    agent_service.get_agent(agent_id).await
}

/// 更新 Agent 单个字段
#[tauri::command]
pub async fn agent_update_field(
    agent_id: UUID,
    field_name: String,
    value: serde_json::Value,
    agent_service: State<'_, AgentService>,
) -> Result<Agent, AppError> {
    let parameter = match field_name.as_str() {
        "name" => {
            let name = value
                .as_str()
                .ok_or_else(|| AppError::validation_error("Invalid name value"))?
                .to_string();
            AgentParameter::Name(name)
        }
        "temperature" => {
            let temp_value = if value.is_null() {
                None
            } else {
                let val = value
                    .as_f64()
                    .ok_or_else(|| AppError::validation_error("Invalid temperature value"))?
                    as f32;
                Some(val)
            };
            AgentParameter::Temperature(temp_value)
        }
        "topP" => {
            let top_p_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_f64()
                        .ok_or_else(|| AppError::validation_error("Invalid top_p value"))?
                        as f32,
                )
            };
            AgentParameter::TopP(top_p_value)
        }
        "topK" => {
            let top_k_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_i64()
                        .ok_or_else(|| AppError::validation_error("Invalid top_k value"))?
                        as i32,
                )
            };
            AgentParameter::TopK(top_k_value)
        }
        "maxTokens" => {
            let max_tokens_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_i64()
                        .ok_or_else(|| AppError::validation_error("Invalid max_tokens value"))?
                        as i32,
                )
            };
            AgentParameter::MaxTokens(max_tokens_value)
        }
        "streaming" => {
            let streaming_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_bool()
                        .ok_or_else(|| AppError::validation_error("Invalid streaming value"))?,
                )
            };
            AgentParameter::Streaming(streaming_value)
        }
        "systemPrompt" => {
            let prompt_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_str()
                        .ok_or_else(|| AppError::validation_error("Invalid system_prompt value"))?
                        .to_string(),
                )
            };
            AgentParameter::SystemPrompt(prompt_value)
        }
        "mcpServers" => {
            let servers = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid mcp_servers value: {}", e))
            })?;
            AgentParameter::McpServers(servers)
        }
        "skills" => {
            let skills = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid skills value: {}", e))
            })?;
            AgentParameter::Skills(skills)
        }
        "reasoning" => {
            let reasoning_value = if value.is_null() {
                None
            } else {
                Some(
                    serde_json::from_value::<AgentReasoningConfig>(value).map_err(|e| {
                        AppError::validation_error(&format!(
                            "Invalid reasoning configuration: {}",
                            e
                        ))
                    })?,
                )
            };
            AgentParameter::Reasoning(reasoning_value)
        }
        _ => {
            return Err(AppError::validation_error(&format!(
                "Unknown field: {}",
                field_name
            )))
        }
    };

    agent_service.update_agent_parameter(agent_id, parameter).await
}

/// 更新 Agent 模型
#[tauri::command]
pub async fn agent_update_model(
    agent_id: UUID,
    model_id: String,
    provider_id: String,
    agent_service: State<'_, AgentService>,
) -> Result<Agent, AppError> {
    agent_service
        .update_agent_parameter(
            agent_id,
            AgentParameter::Model {
                model_id,
                provider_id,
            },
        )
        .await
}

/// 更新 Agent 名称
#[tauri::command]
pub async fn agent_update_name(
    agent_id: UUID,
    name: String,
    agent_service: State<'_, AgentService>,
) -> Result<Agent, AppError> {
    agent_service
        .update_agent_parameter(agent_id, AgentParameter::Name(name))
        .await
}

/// 删除 Agent
#[tauri::command]
pub async fn agent_delete(
    agent_id: UUID,
    agent_service: State<'_, AgentService>,
) -> Result<(), AppError> {
    agent_service.delete_agent(agent_id).await
}
