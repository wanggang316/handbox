// User-defined agent tool ("dynamic tool") domain types.
//
// Mirrors the `tool_definitions` table (migration 069) and the frontend's
// `src/lib/types/toolDefinition.ts`.
//
// A definition is PRESENTATIONAL: it declares what the model may call and what
// arguments it must supply, and the frontend renders the linked GenUI spec with
// those arguments as its state model. Nothing here executes.

use serde::{Deserialize, Serialize};

use super::{Timestamp, UUID};

/// The type of one declared parameter.
///
/// The set is deliberately closed and aligned with the GenUI catalog's prop
/// shapes (`src/lib/components/genui/jsonui/catalog.ts`): a parameter exists to
/// be bound into a spec, so a type that no component can consume would be a
/// parameter the user cannot use. Each maps to a concrete JSON Schema — never a
/// bare `{}` — because the schema is sent to the provider and validated by the
/// agent loop before the tool runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolParamType {
    String,
    Number,
    Boolean,
    /// `string[]` — e.g. `Table.columns`.
    StringList,
    /// `string[][]` — e.g. `Table.rows`.
    StringMatrix,
    /// `{key, value}[]` — e.g. `KeyValue.items`.
    KeyValueList,
}

/// One declared parameter of a user-defined tool.
///
/// `name` doubles as the state path the GenUI spec binds to (`/city`), which is
/// why it is constrained to an identifier by the service layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolParam {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: ToolParamType,
    /// Shown to the model inside the schema; this is what teaches it what to
    /// put in the field.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

/// One user-authored tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: UUID,
    /// Registration name the model calls. Unique across definitions, and
    /// rejected by the service layer when it collides with a built-in id or
    /// looks like an MCP namespace.
    pub name: String,
    /// Human label for the UI (and the tool's `label` in hand-agent).
    pub display_name: String,
    /// Lucide icon name (kebab-case), resolved through `utils/agentIcons.ts`.
    pub icon: Option<String>,
    /// The prompt the model reads: when and why to call this tool.
    pub description: String,
    pub parameters: Vec<ToolParam>,
    /// Linked GenUI spec rendered from the call's arguments. `None` renders the
    /// call as an ordinary tool row instead.
    pub genui_id: Option<UUID>,
    pub enabled: bool,
    /// Display order in settings; also the registration order in a run.
    pub sort_order: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateToolDefinitionRequest {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Vec<ToolParam>,
    #[serde(default)]
    pub genui_id: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

/// Every field optional: an omitted field keeps the stored value, so the
/// settings list can PATCH a single toggle without shipping the whole
/// definition back.
///
/// For the two nullable columns ([`Self::icon`], [`Self::genui_id`]) an **empty
/// string clears** the column, matching `UpdateHookRuleRequest`'s convention —
/// a flat `Option` cannot otherwise distinguish "leave alone" from "set null".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolDefinitionRequest {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Vec<ToolParam>>,
    pub genui_id: Option<String>,
    pub enabled: Option<bool>,
    pub sort_order: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire format is the frontend's contract: `type` (not `paramType`) and
    /// camelCase variant tags.
    #[test]
    fn a_parameter_serializes_with_the_frontend_field_names() {
        let json = serde_json::to_value(ToolParam {
            name: "city".to_string(),
            param_type: ToolParamType::KeyValueList,
            description: "rows".to_string(),
            required: true,
        })
        .unwrap();

        assert_eq!(json["type"], "keyValueList");
        assert_eq!(json["name"], "city");
        assert_eq!(json["required"], true);
    }

    /// Persisted rows carry only `name`/`type`; the optional halves must decode
    /// to their defaults rather than failing the whole row.
    #[test]
    fn a_parameter_decodes_without_its_optional_fields() {
        let param: ToolParam =
            serde_json::from_str(r#"{"name":"city","type":"string"}"#).expect("decodes");
        assert_eq!(param.param_type, ToolParamType::String);
        assert_eq!(param.description, "");
        assert!(!param.required);
    }
}
