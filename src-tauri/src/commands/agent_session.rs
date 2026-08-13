// Agent-session CRUD commands, delegating to `AgentSessionService`.
//
// Two data sources coexist (transparent to the frontend):
//  - Session CONFIG (model/provider/tools/project linkage, …) is authoritative
//    in SQLite (`agent_sessions` rows) — e.g. the projectId used for grouping
//    lives only there.
//  - Session ACTIVITY (messageCount / lastMessageAt / title) and the transcript
//    are authoritative in JSONL (written by the coding-agent SessionManager).
//    `agent_session_list` overlays JSONL activity onto the SQLite rows;
//    `agent_session_messages` reads the JSONL directly, falling back to the
//    SQLite transcript for legacy sessions without a JSONL file.

use crate::models::AppError;
use crate::services::{
    abort_run, agent_jsonl_store, title_gen, AgentSessionParameter, AgentSessionService,
    ProviderService,
};
use crate::storage::types::{
    AgentSession, AgentSessionMessage, CreateAgentSessionRequest, InstantiateAgentSessionRequest,
    UUID,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn agent_session_create(
    request: CreateAgentSessionRequest,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service.create_session(request).await
}

/// Instantiates a session from an AgentDefinition (incl. built-in chat/coding):
/// the definition supplies the capability set and defaults, `overrides` carries
/// instantiation-time working dir / model / provider / name. See
/// [`AgentSessionService::create_session_from_definition`].
#[tauri::command]
pub async fn agent_session_create_from_definition(
    definition_id: UUID,
    overrides: Option<InstantiateAgentSessionRequest>,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service
        .create_session_from_definition(definition_id, overrides.unwrap_or_default())
        .await
}

/// Repoints an existing EMPTY session to another AgentDefinition in place (no
/// new session row). The frontend only calls this while the session has no
/// messages: switching agents re-snapshots the capability set and rewrites
/// provenance while keeping the session id and transcript. See
/// [`AgentSessionService::reinstantiate_from_definition`].
#[tauri::command]
pub async fn agent_session_reinstantiate_from_definition(
    session_id: UUID,
    definition_id: UUID,
    overrides: Option<InstantiateAgentSessionRequest>,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service
        .reinstantiate_from_definition(session_id, definition_id, overrides.unwrap_or_default())
        .await
}

/// SQLite supplies the config rows (and the ordering: updated_at DESC); JSONL
/// supplies activity metadata (messageCount / lastMessageAt / title), overlaid
/// per row so the sidebar reflects the real transcript. Sessions without a JSONL
/// file keep their SQLite fields.
#[tauri::command]
pub async fn agent_session_list(
    limit: Option<i32>,
    offset: Option<i32>,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSession>, AppError> {
    let mut sessions = agent_session_service.list_sessions(limit, offset).await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    for session in sessions.iter_mut() {
        overlay_jsonl_activity(session, &app_data_dir);
    }
    Ok(sessions)
}

/// Same as `agent_session_list`: overlays JSONL activity onto the SQLite row so
/// a post-run refresh (frontend `getAgentSession`) sees the real messageCount /
/// lastMessageAt — the SQLite append path does not update those columns.
#[tauri::command]
pub async fn agent_session_get(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let mut session = agent_session_service.get_session(session_id).await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    overlay_jsonl_activity(&mut session, &app_data_dir);
    Ok(session)
}

/// Resolves the Tauri app data dir (the JSONL persistence root).
fn resolve_app_data_dir(app_handle: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal_error(&format!("failed to resolve app data dir: {e}")))
}

/// Overlays JSONL activity metadata onto a SQLite session row in place.
///
/// With a JSONL file, messageCount / lastMessageAt come from JSONL (the
/// activity authority), and a JSONL session label overrides name. No JSONL file
/// (legacy session) or a read failure keeps the SQLite fields — one bad file
/// must never take down the whole list.
fn overlay_jsonl_activity(session: &mut AgentSession, app_data_dir: &std::path::Path) {
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), app_data_dir);
    match agent_jsonl_store::session_activity(app_data_dir, &cwd, &session.id) {
        Ok(Some(activity)) => {
            session.message_count = activity.message_count;
            session.last_message_at = activity.last_message_at;
            if let Some(name) = activity.name {
                session.name = name;
            }
        }
        // No JSONL (legacy session): keep the SQLite values.
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(
                session_id = %session.id,
                "failed to read JSONL activity, keeping SQLite values: {e}"
            );
        }
    }
}

/// Renames a session: dual-write of the SQLite name and a JSONL label.
///
/// SQLite `name` is the fallback name source, but the list/get overlay replaces
/// it whenever the session's JSONL carries a label — writing SQLite alone would
/// be visually undone by an older auto-generated JSONL label. So after the
/// SQLite write succeeds, a label is **appended** to the session's JSONL (latest
/// label wins), making the overlay reflect the user's new name.
///
/// The JSONL write is best-effort: a failure degrades to a warn log without
/// failing the rename — SQLite remains the fallback name source, matching the
/// overlay's read-failure posture. Returns the **overlaid** session (consistent
/// with list/get) so the frontend gets the new name directly.
#[tauri::command]
pub async fn agent_session_rename(
    session_id: UUID,
    name: String,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    // SQLite (fallback name source) writes first.
    let mut session = agent_session_service
        .rename_session(session_id, name.clone())
        .await?;

    // Best-effort JSONL label write; a failure must not fail the rename.
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);
    // Pass the session's real created_at so a first-ever rename (which may be the
    // session's first on-disk write) seeds the JSONL header with the creation
    // time rather than the rename moment, keeping createdAt == header.timestamp.
    if let Err(e) =
        agent_jsonl_store::append_label(&app_data_dir, &cwd, &session.id, &name, session.created_at)
    {
        tracing::warn!(
            session_id = %session.id,
            "failed to write JSONL label on rename, keeping SQLite name: {e}"
        );
    }

    // Return the overlaid session: the just-written label makes name the new name.
    overlay_jsonl_activity(&mut session, &app_data_dir);
    Ok(session)
}

/// Generates a session title: one LLM completion using the session's own
/// model/provider, then the same persistence path as rename (SQLite name +
/// JSONL label + overlaid return). Shared by the automatic (per the
/// `session.titleGeneration` rule) and manual (context menu) paths.
///
/// `scope` picks the source text — the first user message (default) or the
/// conversation so far, for re-titling a session as its topic evolves.
///
/// Failures (no provider/model, no user message, model error / empty result)
/// return an AppError; the frontend surfaces it and does **not** rename.
#[tauri::command]
pub async fn agent_session_generate_title(
    session_id: UUID,
    scope: Option<title_gen::TitleScope>,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
    provider_service: State<'_, ProviderService>,
) -> Result<AgentSession, AppError> {
    let scope = scope.unwrap_or_default();
    let session = agent_session_service.get_session(session_id.clone()).await?;
    let provider_id = session
        .provider_id
        .clone()
        .ok_or_else(|| AppError::validation_error("会话未选择供应商，无法生成标题"))?;
    let model_id = session
        .model_id
        .clone()
        .ok_or_else(|| AppError::validation_error("会话未选择模型，无法生成标题"))?;
    let provider = provider_service.get_provider(&provider_id).await?;

    // User message text (the JSONL is the transcript authority). Messages
    // carrying no text at all (image-only turns) are skipped rather than
    // aborting the whole generation.
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);
    let user_texts: Vec<String> =
        agent_jsonl_store::load_transcript(&app_data_dir, &cwd, &session.id)?
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.role == "user")
            .filter_map(|m| extract_user_text(&m.payload))
            .collect();
    let source_text = match scope {
        title_gen::TitleScope::FirstMessage => user_texts.into_iter().next(),
        title_gen::TitleScope::Conversation => {
            let joined = build_conversation_source(user_texts);
            (!joined.is_empty()).then_some(joined)
        }
    }
    .ok_or_else(|| AppError::validation_error("该会话还没有可用于生成标题的消息"))?;

    let title = title_gen::generate_title(
        &provider.provider_type,
        &model_id,
        &provider.base_url,
        &provider.api_key,
        &source_text,
        scope,
    )
    .await
    .map_err(|e| {
        // The real cause is hidden behind the frontend's generic hint; log it.
        tracing::warn!(session_id = %session.id, error = %e, "session title generation failed");
        e
    })?;

    // Same persistence as agent_session_rename: SQLite name first, then a
    // best-effort JSONL label, returning the overlaid session.
    let mut session = agent_session_service
        .rename_session(session_id, title.clone())
        .await?;
    if let Err(e) = agent_jsonl_store::append_label(
        &app_data_dir,
        &cwd,
        &session.id,
        &title,
        session.created_at,
    ) {
        tracing::warn!(
            session_id = %session.id,
            "failed to write JSONL label on generated title, keeping SQLite name: {e}"
        );
    }
    overlay_jsonl_activity(&mut session, &app_data_dir);
    Ok(session)
}

/// Trailing user messages fed to a conversation-scope title. Older turns are
/// dropped: the recent ones decide what the session is about now, and the char
/// cap inside `title_gen` is the final size guard.
const MAX_CONVERSATION_MESSAGES: usize = 12;

/// Joins the last [`MAX_CONVERSATION_MESSAGES`] user messages, oldest to
/// newest, into one blank-line-separated block for a conversation-scope title.
fn build_conversation_source(mut texts: Vec<String>) -> String {
    if texts.len() > MAX_CONVERSATION_MESSAGES {
        texts.drain(..texts.len() - MAX_CONVERSATION_MESSAGES);
    }
    texts.join("\n\n")
}

/// Extracts plain text from a persisted user-message payload (a serialized
/// hand-ai `Message::User`); `content` may be a string (`UserContent::Text`) or
/// an array of content blocks (`UserContent::Blocks`).
fn extract_user_text(payload: &serde_json::Value) -> Option<String> {
    match payload.get("content")? {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        serde_json::Value::Array(blocks) => {
            let mut out = String::new();
            for block in blocks {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        if !out.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(text);
                    }
                }
            }
            let out = out.trim().to_string();
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        _ => None,
    }
}

/// Pins / unpins a session in the sidebar.
///
/// Its own command rather than an `agent_session_update_field` case: the flag is
/// written as a single column so a concurrent field edit cannot revert it (see
/// [`AgentSessionRepository::set_session_pinned`]). Returns the overlaid session,
/// consistent with rename/get, so the frontend can swap the list entry in place.
#[tauri::command]
pub async fn agent_session_set_pinned(
    session_id: UUID,
    pinned: bool,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let mut session = agent_session_service
        .set_session_pinned(session_id, pinned)
        .await?;
    overlay_jsonl_activity(&mut session, &resolve_app_data_dir(&app_handle)?);
    Ok(session)
}

/// Archives / unarchives a session. Same shape as [`agent_session_set_pinned`];
/// nothing is deleted, so unarchiving restores the session untouched.
#[tauri::command]
pub async fn agent_session_set_archived(
    session_id: UUID,
    archived: bool,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let mut session = agent_session_service
        .set_session_archived(session_id, archived)
        .await?;
    overlay_jsonl_activity(&mut session, &resolve_app_data_dir(&app_handle)?);
    Ok(session)
}

/// Updates a single session field (mirrors `agent_update_field`).
#[tauri::command]
pub async fn agent_session_update_field(
    session_id: UUID,
    field_name: String,
    value: serde_json::Value,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let parameter = parse_session_parameter(&field_name, value)?;

    agent_session_service
        .update_session_field(session_id, parameter)
        .await
}

/// Parses an IPC field name + JSON value into an `AgentSessionParameter`.
///
/// Unknown fields (including the unsupported `"enabledSkills"`) return
/// VALIDATION_ERROR; the parameter is never constructed and the service is
/// never called, so nothing is written.
fn parse_session_parameter(
    field_name: &str,
    value: serde_json::Value,
) -> Result<AgentSessionParameter, AppError> {
    let parameter = match field_name {
        "name" => {
            let name = value
                .as_str()
                .ok_or_else(|| AppError::validation_error("Invalid name value"))?
                .to_string();
            AgentSessionParameter::Name(name)
        }
        "modelId" => AgentSessionParameter::ModelId(parse_optional_string(&value, "model_id")?),
        "providerId" => {
            AgentSessionParameter::ProviderId(parse_optional_string(&value, "provider_id")?)
        }
        "systemPrompt" => {
            AgentSessionParameter::SystemPrompt(parse_optional_string(&value, "system_prompt")?)
        }
        "thinkingLevel" => {
            AgentSessionParameter::ThinkingLevel(parse_optional_string(&value, "thinking_level")?)
        }
        "temperature" => {
            let temp_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_f64()
                        .ok_or_else(|| AppError::validation_error("Invalid temperature value"))?
                        as f32,
                )
            };
            AgentSessionParameter::Temperature(temp_value)
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
            AgentSessionParameter::MaxTokens(max_tokens_value)
        }
        "workingDir" => {
            AgentSessionParameter::WorkingDir(parse_optional_string(&value, "working_dir")?)
        }
        "enabledTools" => {
            let tools = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid enabled_tools value: {}", e))
            })?;
            AgentSessionParameter::EnabledTools(tools)
        }
        "mcpServers" => {
            let servers = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid mcp_servers value: {}", e))
            })?;
            AgentSessionParameter::McpServers(servers)
        }
        "toolExecutionMode" => AgentSessionParameter::ToolExecutionMode(parse_optional_string(
            &value,
            "tool_execution_mode",
        )?),
        _ => {
            return Err(AppError::validation_error(&format!(
                "Unknown field: {}",
                field_name
            )))
        }
    };
    Ok(parameter)
}

/// Deletes a session, including its JSONL transcript file.
///
/// Order: abort any active run first (`abort_run` is a no-op when idle) so no
/// `agent_stream_event` for the deleted session reaches the frontend afterwards;
/// then best-effort delete the JSONL file (deleting only the SQLite row would
/// leave an orphan `<id>.jsonl` on disk); finally delete the SQLite row — the
/// **authority** that decides whether the list still shows the session. Even if
/// the JSONL delete fails (warn), a successful SQLite delete removes the row.
///
/// Fetches the session first for `working_dir` → JSONL cwd; a missing session
/// makes `get_session` return NOT_FOUND.
#[tauri::command]
pub async fn agent_session_delete(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<(), AppError> {
    let session = agent_session_service
        .get_session(session_id.clone())
        .await?;

    abort_run(&session_id);

    // Best-effort JSONL cleanup; must not block the authoritative SQLite delete.
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);
    if let Err(e) = agent_jsonl_store::delete_session_file(&app_data_dir, &cwd, &session_id) {
        tracing::warn!(
            session_id = %session_id,
            "failed to delete JSONL transcript file on delete, removing SQLite row anyway: {e}"
        );
    }

    agent_session_service.delete_session(session_id).await
}

/// Returns the session transcript; the JSONL is the source of truth.
///
/// Reads the session's JSONL transcript (`<app_data_dir>/sessions/
/// <flattened-cwd>/<id>.jsonl`, restored via SessionManager `build_context`,
/// incl. tool calls and thinking blocks embedded in assistant content blocks).
/// Falls back to SQLite for legacy sessions without a JSONL file, and on a hard
/// JSONL read failure (rare, e.g. a file corrupt enough that open errors) so a
/// single bad file cannot blank the whole timeline.
#[tauri::command]
pub async fn agent_session_messages(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSessionMessage>, AppError> {
    let session = agent_session_service
        .get_session(session_id.clone())
        .await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);

    match agent_jsonl_store::load_transcript(&app_data_dir, &cwd, &session_id) {
        Ok(Some(rows)) => Ok(rows),
        // No JSONL file: legacy session → fall back to the SQLite transcript.
        Ok(None) => agent_session_service.list_messages(session_id).await,
        // JSONL exists but read hard-failed: log and fall back to SQLite.
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                "failed to read JSONL transcript, falling back to SQLite: {e}"
            );
            agent_session_service.list_messages(session_id).await
        }
    }
}

/// null -> None, string -> Some, anything else -> validation error.
fn parse_optional_string(
    value: &serde_json::Value,
    field: &str,
) -> Result<Option<String>, AppError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(
            value
                .as_str()
                .ok_or_else(|| AppError::validation_error(&format!("Invalid {} value", field)))?
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `"enabledSkills"` is not a known field — every value shape falls into the
    /// Unknown-field VALIDATION_ERROR. The parameter is never constructed, the
    /// service is never invoked, so no row can be written.
    #[test]
    fn update_field_enabled_skills_is_unknown_field() {
        for value in [
            serde_json::json!(["a", "b"]),
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!("not-an-array"),
        ] {
            // AgentSessionParameter is not Debug, so match instead of expect_err.
            match parse_session_parameter("enabledSkills", value.clone()) {
                Ok(_) => panic!("enabledSkills must be rejected as unknown: {}", value),
                Err(err) => {
                    assert_eq!(
                        err.code, "VALIDATION_ERROR",
                        "rejection must be a VALIDATION_ERROR for {}",
                        value
                    );
                    assert!(
                        err.message.contains("Unknown field: enabledSkills"),
                        "must fall into the Unknown-field branch, got: {}",
                        err.message
                    );
                }
            }
        }
    }

    /// A conversation source keeps only the newest `MAX_CONVERSATION_MESSAGES`
    /// turns, in chronological order, separated by a blank line.
    #[test]
    fn conversation_source_keeps_the_newest_turns_in_order() {
        let texts: Vec<String> = (0..MAX_CONVERSATION_MESSAGES + 3)
            .map(|i| format!("m{i}"))
            .collect();
        let source = build_conversation_source(texts);

        assert!(!source.contains("m2"), "the oldest turns are dropped");
        assert!(source.starts_with("m3"), "the window starts at the cut");
        assert!(
            source.ends_with(&format!("m{}", MAX_CONVERSATION_MESSAGES + 2)),
            "the newest turn is last"
        );
        assert_eq!(
            source.split("\n\n").count(),
            MAX_CONVERSATION_MESSAGES,
            "exactly the window size survives"
        );
    }

    /// Short conversations pass through whole; an empty transcript yields an
    /// empty source, which the caller turns into a VALIDATION_ERROR.
    #[test]
    fn conversation_source_handles_short_and_empty_input() {
        assert_eq!(
            build_conversation_source(vec!["a".to_string(), "b".to_string()]),
            "a\n\nb"
        );
        assert_eq!(build_conversation_source(Vec::new()), "");
    }

    /// The other field mappings — thinkingLevel / enabledTools / workingDir /
    /// modelId — parse into their parameter variants.
    #[test]
    fn other_field_mappings_survive_enabled_skills_removal() {
        match parse_session_parameter("thinkingLevel", serde_json::json!("high")) {
            Ok(AgentSessionParameter::ThinkingLevel(Some(level))) => assert_eq!(level, "high"),
            _ => panic!("thinkingLevel must map to ThinkingLevel(Some)"),
        }

        match parse_session_parameter("enabledTools", serde_json::json!(["read", "write"])) {
            Ok(AgentSessionParameter::EnabledTools(tools)) => {
                assert_eq!(tools, vec!["read".to_string(), "write".to_string()]);
            }
            _ => panic!("enabledTools must map to EnabledTools"),
        }

        match parse_session_parameter("workingDir", serde_json::json!("/tmp")) {
            Ok(AgentSessionParameter::WorkingDir(Some(dir))) => assert_eq!(dir, "/tmp"),
            _ => panic!("workingDir must map to WorkingDir(Some)"),
        }

        match parse_session_parameter("modelId", serde_json::json!(null)) {
            Ok(AgentSessionParameter::ModelId(None)) => {}
            _ => panic!("modelId null must map to ModelId(None)"),
        }
    }
}
