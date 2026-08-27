//! Tool-definition service: a thin layer over [`ToolDefinitionRepository`] that
//! owns the write-time concerns the storage layer deliberately leaves out —
//! timestamps, and the name/parameter rules that keep a definition callable.
//!
//! Names matter more here than in most CRUD: a definition's `name` is what the
//! model calls, and it shares ONE namespace with the coding-agent built-ins, the
//! extension tools, and the `mcp__*` bindings. A collision would silently
//! shadow (or be shadowed by) another tool in `extra_tools`, so it is rejected
//! at write time rather than discovered at run time.

use std::sync::Arc;

use crate::models::AppError;
use crate::services::extensions::{EXTENSION_TOOL_IDS, TOOL_SKILL};
use crate::storage::types::{
    CreateToolDefinitionRequest, ToolDefinition, ToolParam, UpdateToolDefinitionRequest,
};
use crate::storage::{Database, ToolDefinitionRepository};

#[derive(Clone)]
pub struct ToolDefinitionService {
    repository: Arc<ToolDefinitionRepository>,
}

/// Milliseconds since the epoch; the storage layer takes the timestamp rather
/// than reading the clock so tests can pin it.
fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The coding-agent built-ins `select_enabled_tools` resolves. Mirrors
/// `src/lib/constants/builtinToolIds.ts`; kept here (rather than imported from
/// the crate) because it is a *naming* rule, not a registration one.
const CODING_AGENT_TOOL_IDS: [&str; 7] = ["read", "write", "edit", "bash", "grep", "find", "ls"];

/// Namespace prefix of a per-session MCP tool (`mcp__<serverId>__<tool>`).
const MCP_PREFIX: &str = "mcp__";

/// Longest accepted registration name. Well under any provider's limit, and
/// short enough that the settings list stays readable.
const MAX_NAME_LEN: usize = 48;

/// A registration name must be a lower-snake identifier: it goes into a tool
/// schema, so anything a provider might reject or a model might mistype is out.
fn validate_name_shape(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(AppError::validation_error(&format!(
            "Tool name must be 1-{MAX_NAME_LEN} characters"
        )));
    }
    let mut chars = name.chars();
    let first_is_lower_alpha = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest_is_identifier =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !first_is_lower_alpha || !rest_is_identifier {
        return Err(AppError::with_hint(
            "VALIDATION_ERROR",
            "Tool name must start with a lowercase letter and contain only lowercase letters, digits and underscores",
            "例如 weather_card",
        ));
    }
    Ok(())
}

/// Reject names that would collide with a tool the runtime registers itself.
fn validate_name_is_free(name: &str) -> Result<(), AppError> {
    if name.starts_with(MCP_PREFIX) {
        return Err(AppError::with_hint(
            "VALIDATION_ERROR",
            "Tool name must not start with `mcp__`; that namespace belongs to MCP servers",
            "换一个名字",
        ));
    }
    let reserved = CODING_AGENT_TOOL_IDS
        .iter()
        .chain(EXTENSION_TOOL_IDS.iter())
        .chain(std::iter::once(&TOOL_SKILL))
        .any(|id| *id == name);
    if reserved {
        return Err(AppError::with_hint(
            "VALIDATION_ERROR",
            &format!("`{name}` is a built-in tool name"),
            "换一个名字",
        ));
    }
    Ok(())
}

/// Parameter names double as the state path a GenUI spec binds to (`/city`), so
/// they follow the same identifier rule and must be unique within one tool.
fn validate_parameters(parameters: &[ToolParam]) -> Result<(), AppError> {
    let mut seen: Vec<&str> = Vec::with_capacity(parameters.len());
    for param in parameters {
        let name = param.name.as_str();
        if name.is_empty() {
            return Err(AppError::validation_error("A parameter needs a name"));
        }
        let mut chars = name.chars();
        let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
        let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !first_ok || !rest_ok {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("Parameter name `{name}` must be a letter followed by letters, digits or underscores"),
                "参数名同时是 GenUI 的绑定路径",
            ));
        }
        if seen.contains(&name) {
            return Err(AppError::validation_error(&format!(
                "Duplicate parameter name: {name}"
            )));
        }
        seen.push(name);
    }
    Ok(())
}

impl ToolDefinitionService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: Arc::new(ToolDefinitionRepository::new(db)),
        }
    }

    /// Enabled definitions in display order — the snapshot a run registers.
    pub async fn list_enabled(&self) -> Result<Vec<ToolDefinition>, AppError> {
        self.repository.list_enabled().await
    }

    /// Every definition, enabled or not, for the settings UI.
    pub async fn list(&self) -> Result<Vec<ToolDefinition>, AppError> {
        self.repository.list().await
    }

    pub async fn get(&self, id: &str) -> Result<ToolDefinition, AppError> {
        self.repository
            .get(id)
            .await?
            .ok_or_else(|| AppError::not_found(&format!("Tool definition not found: {}", id)))
    }

    pub async fn create(
        &self,
        request: CreateToolDefinitionRequest,
    ) -> Result<ToolDefinition, AppError> {
        let name = request.name.trim().to_string();
        validate_name_shape(&name)?;
        validate_name_is_free(&name)?;
        self.reject_taken_name(&name, None).await?;
        validate_parameters(&request.parameters)?;

        self.repository
            .create(
                CreateToolDefinitionRequest {
                    name,
                    display_name: display_name_or_fallback(&request.display_name, &request.name),
                    ..request
                },
                now_ms(),
            )
            .await
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateToolDefinitionRequest,
    ) -> Result<ToolDefinition, AppError> {
        let mut request = request;
        if let Some(name) = request.name.as_ref() {
            let name = name.trim().to_string();
            validate_name_shape(&name)?;
            validate_name_is_free(&name)?;
            self.reject_taken_name(&name, Some(id)).await?;
            request.name = Some(name);
        }
        if let Some(parameters) = request.parameters.as_ref() {
            validate_parameters(parameters)?;
        }

        self.repository.update(id, request, now_ms()).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        self.repository.delete(id).await
    }

    /// Uniqueness check ahead of the UNIQUE index, so the UI gets a field-level
    /// message instead of an opaque SQL constraint error. `keep` is the id being
    /// updated — a definition keeping its own name is not a collision.
    async fn reject_taken_name(&self, name: &str, keep: Option<&str>) -> Result<(), AppError> {
        if let Some(existing) = self.repository.get_by_name(name).await? {
            if keep != Some(existing.id.as_str()) {
                return Err(AppError::with_hint(
                    "VALIDATION_ERROR",
                    &format!("A tool named `{name}` already exists"),
                    "换一个名字",
                ));
            }
        }
        Ok(())
    }
}

/// A tool with no label would render as a blank row; fall back to the
/// registration name rather than showing nothing.
fn display_name_or_fallback(display_name: &str, name: &str) -> String {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        name.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::ToolParamType;
    use tempfile::TempDir;

    async fn service() -> (ToolDefinitionService, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(
            Database::new(&dir.path().join("test.db"))
                .await
                .expect("database"),
        );
        (ToolDefinitionService::new(db), dir)
    }

    fn request(name: &str) -> CreateToolDefinitionRequest {
        CreateToolDefinitionRequest {
            name: name.to_string(),
            display_name: "Weather card".to_string(),
            icon: None,
            description: "Call when the user asks about the weather".to_string(),
            parameters: vec![ToolParam {
                name: "city".to_string(),
                param_type: ToolParamType::String,
                description: String::new(),
                required: true,
            }],
            genui_id: None,
            sort_order: None,
        }
    }

    #[tokio::test]
    async fn a_valid_definition_is_created() {
        let (service, _dir) = service().await;
        let created = service.create(request("weather_card")).await.unwrap();
        assert_eq!(created.name, "weather_card");
        assert!(created.enabled);
    }

    #[tokio::test]
    async fn a_name_is_trimmed_before_it_is_stored() {
        let (service, _dir) = service().await;
        let created = service.create(request("  weather_card  ")).await.unwrap();
        assert_eq!(created.name, "weather_card");
    }

    #[tokio::test]
    async fn a_malformed_name_is_rejected() {
        let (service, _dir) = service().await;
        for name in [
            "",
            "Weather",
            "1card",
            "weather-card",
            "weather card",
            "wéather",
        ] {
            let err = service
                .create(request(name))
                .await
                .expect_err(&format!("`{name}` is not a valid registration name"));
            assert_eq!(err.code, "VALIDATION_ERROR");
        }
        let too_long = "a".repeat(MAX_NAME_LEN + 1);
        assert_eq!(
            service.create(request(&too_long)).await.unwrap_err().code,
            "VALIDATION_ERROR"
        );
    }

    /// The collision that matters: a definition shadowing a tool the runtime
    /// registers itself would be resolved by whichever landed in `extra_tools`
    /// last — never discovered, always wrong.
    #[tokio::test]
    async fn a_name_taken_by_the_runtime_is_rejected() {
        let (service, _dir) = service().await;
        for name in [
            "read",
            "write",
            "edit",
            "bash",
            "grep",
            "find",
            "ls",
            "web_search",
            "render_card",
            "render_app",
            "ask_question",
            "skill",
            "mcp__srv__tool",
        ] {
            let err = service
                .create(request(name))
                .await
                .expect_err(&format!("`{name}` belongs to the runtime"));
            assert_eq!(err.code, "VALIDATION_ERROR");
        }
    }

    #[tokio::test]
    async fn a_duplicate_name_is_rejected_with_a_field_level_message() {
        let (service, _dir) = service().await;
        service.create(request("weather_card")).await.unwrap();

        let err = service
            .create(request("weather_card"))
            .await
            .expect_err("names are unique");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("already exists"));
    }

    /// Renaming must not trip over the definition's own row.
    #[tokio::test]
    async fn a_definition_may_keep_its_own_name_on_update() {
        let (service, _dir) = service().await;
        let created = service.create(request("weather_card")).await.unwrap();

        service
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    name: Some("weather_card".to_string()),
                    display_name: Some("Weather".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("keeping the same name is not a collision");

        let other = service.create(request("other_card")).await.unwrap();
        let err = service
            .update(
                &other.id,
                UpdateToolDefinitionRequest {
                    name: Some("weather_card".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("taking another definition's name is");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn parameter_names_are_validated_and_unique() {
        let (service, _dir) = service().await;
        let bad = |param_name: &str| CreateToolDefinitionRequest {
            parameters: vec![ToolParam {
                name: param_name.to_string(),
                param_type: ToolParamType::String,
                description: String::new(),
                required: false,
            }],
            ..request("tool_a")
        };
        for param_name in ["", "1city", "city name", "city-name"] {
            assert_eq!(
                service.create(bad(param_name)).await.unwrap_err().code,
                "VALIDATION_ERROR"
            );
        }

        let duplicate = CreateToolDefinitionRequest {
            parameters: vec![
                ToolParam {
                    name: "city".to_string(),
                    param_type: ToolParamType::String,
                    description: String::new(),
                    required: false,
                },
                ToolParam {
                    name: "city".to_string(),
                    param_type: ToolParamType::Number,
                    description: String::new(),
                    required: false,
                },
            ],
            ..request("tool_b")
        };
        let err = service.create(duplicate).await.unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("Duplicate"));
    }

    /// Parameters sent on an update are validated too — otherwise the editor
    /// could write a duplicate that only fails when the model calls the tool.
    #[tokio::test]
    async fn an_update_validates_the_parameters_it_carries() {
        let (service, _dir) = service().await;
        let created = service.create(request("weather_card")).await.unwrap();

        let err = service
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    parameters: Some(vec![ToolParam {
                        name: "1bad".to_string(),
                        param_type: ToolParamType::String,
                        description: String::new(),
                        required: false,
                    }]),
                    ..Default::default()
                },
            )
            .await
            .expect_err("a malformed parameter name is rejected on update too");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn a_blank_display_name_falls_back_to_the_registration_name() {
        let (service, _dir) = service().await;
        let created = service
            .create(CreateToolDefinitionRequest {
                display_name: "   ".to_string(),
                ..request("weather_card")
            })
            .await
            .unwrap();
        assert_eq!(created.display_name, "weather_card");
    }

    #[tokio::test]
    async fn list_enabled_excludes_a_disabled_definition() {
        let (service, _dir) = service().await;
        let created = service.create(request("weather_card")).await.unwrap();
        service
            .update(
                &created.id,
                UpdateToolDefinitionRequest {
                    enabled: Some(false),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(service.list_enabled().await.unwrap().is_empty());
        assert_eq!(service.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_reports_a_missing_definition() {
        let (service, _dir) = service().await;
        let err = service.get("missing").await.unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
    }
}
