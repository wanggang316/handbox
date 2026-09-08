// User-defined tool ("dynamic tool") management IPC commands, plus the
// read-only catalog of the tools HandBox registers itself.

use crate::models::AppError;
use crate::services::extensions::{ask_question, render_app, render_card, web_search, TOOL_SKILL};
use crate::services::ToolDefinitionService;
use crate::storage::types::{
    CreateToolDefinitionRequest, ToolDefinition, UpdateToolDefinitionRequest,
};
use hand_coding_agent::tools::create_default_tools;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn tool_definition_list(
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<Vec<ToolDefinition>, AppError> {
    tool_definition_service.list().await
}

#[tauri::command]
pub async fn tool_definition_get(
    tool_id: String,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.get(&tool_id).await
}

#[tauri::command]
pub async fn tool_definition_create(
    request: CreateToolDefinitionRequest,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.create(request).await
}

#[tauri::command]
pub async fn tool_definition_update(
    tool_id: String,
    request: UpdateToolDefinitionRequest,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.update(&tool_id, request).await
}

#[tauri::command]
pub async fn tool_definition_delete(
    tool_id: String,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<(), AppError> {
    tool_definition_service.delete(&tool_id).await
}

/// One built-in tool as the settings detail view shows it.
///
/// This is what the MODEL is told about the tool — the same description and
/// schema `AgentTool` carries into a run — rather than a hand-written copy that
/// would drift from it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinToolInfo {
    /// Registration name; matches an id in `builtinToolIds.ts`.
    pub name: String,
    /// hand-agent's UI label for the tool.
    pub label: String,
    pub description: String,
    /// JSON Schema of the tool's arguments.
    pub parameters: serde_json::Value,
}

impl From<hand_agent::AgentTool> for BuiltinToolInfo {
    fn from(tool: hand_agent::AgentTool) -> Self {
        Self {
            name: tool.name,
            label: tool.label,
            description: tool.description,
            parameters: tool.parameters,
        }
    }
}

/// Read-only catalog of the tools HandBox registers itself: the coding-agent
/// built-ins plus the extension tools.
///
/// The extension tools are constructed with inert configuration — no API key,
/// no question emitter — because only their name, label, description and schema
/// are read; nothing here is ever executed. `skill` is deliberately absent: it
/// gates the coding-agent's skill pipeline rather than registering a tool of its
/// own, so it has no schema to show.
#[tauri::command]
pub async fn agent_tool_catalog(app: AppHandle) -> Result<Vec<BuiltinToolInfo>, AppError> {
    // create_default_tools roots the file tools in a cwd. Nothing is executed,
    // but it must be a real directory, and app_data_dir is the same fallback a
    // session with no working directory uses.
    let cwd = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal_error(&format!("failed to resolve app data dir: {e}")))?;

    let mut catalog: Vec<BuiltinToolInfo> = create_default_tools(&cwd)
        .into_iter()
        .map(BuiltinToolInfo::from)
        .collect();

    catalog.push(web_search::create_web_search_tool(String::new(), String::new()).into());
    catalog.push(render_card::make_render_card_tool().into());
    catalog.push(render_app::make_render_app_tool().into());
    catalog.push(ask_question::make_ask_question_tool(String::new(), None).into());
    debug_assert!(
        !catalog.iter().any(|tool| tool.name == TOOL_SKILL),
        "`skill` gates a pipeline and registers no tool of its own"
    );

    Ok(catalog)
}
