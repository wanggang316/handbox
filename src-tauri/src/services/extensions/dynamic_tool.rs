//! dynamic_tool — user-defined tools, built from a stored [`ToolDefinition`].
//!
//! PRESENTATIONAL, exactly like `render_card`: execution only acknowledges. The
//! arguments the model supplies ARE the payload — they already travel in the
//! toolcall block and persist with the transcript, so the frontend renders the
//! definition's linked GenUI spec from them with no extra IPC and nothing
//! transported through the result.
//!
//! Unlike the other extension tools, these carry no fixed id: their names come
//! from user rows, so `build_agent_session` learns them from the `extra_tools`
//! it is handed rather than from [`super::EXTENSION_TOOL_IDS`].

use hand_agent::{AgentTool, ToolResult};
use serde_json::{json, Map, Value};

use crate::storage::types::{ToolDefinition, ToolParam, ToolParamType};

/// Told to the model when the call renders something. Without it, models narrate
/// the card's contents again in prose, which reads as a duplicate answer.
const RENDERED_SUFFIX: &str = "\n\nCalling this tool renders the result to the user directly — do \
                               not repeat its content in your text reply.";

/// Result text for a call that rendered a card.
const RENDERED_ACK: &str = "Rendered to the user.";
/// Result text for a definition with no linked GenUI: the call is recorded in
/// the transcript but shows the user nothing, and saying so plainly beats
/// letting the model believe it displayed something.
const NOT_RENDERED_ACK: &str =
    "Recorded. This tool has no view linked yet, so nothing was shown to the user.";

/// JSON Schema for one parameter type.
///
/// Every arm yields a CONCRETE schema — never a bare `{}` — because this is sent
/// to the provider and compiled by the agent loop's `validate_tool_args`, which
/// is what lets the frontend trust the argument shapes it renders.
fn param_type_schema(param_type: ToolParamType) -> Value {
    match param_type {
        ToolParamType::String => json!({ "type": "string" }),
        ToolParamType::Number => json!({ "type": "number" }),
        ToolParamType::Boolean => json!({ "type": "boolean" }),
        ToolParamType::StringList => json!({
            "type": "array",
            "items": { "type": "string" }
        }),
        ToolParamType::StringMatrix => json!({
            "type": "array",
            "items": { "type": "array", "items": { "type": "string" } }
        }),
        ToolParamType::KeyValueList => json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "value": { "type": "string" }
                },
                "required": ["key", "value"]
            }
        }),
    }
}

/// Compile the declared parameters into the tool's JSON Schema.
///
/// `required` is omitted entirely when nothing is required: an empty array is
/// legal JSON Schema but some providers reject it.
pub fn params_to_json_schema(params: &[ToolParam]) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for param in params {
        let mut schema = param_type_schema(param.param_type);
        let description = param.description.trim();
        if !description.is_empty() {
            schema["description"] = json!(description);
        }
        properties.insert(param.name.clone(), schema);
        if param.required {
            required.push(json!(param.name));
        }
    }

    let mut schema = json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = Value::Array(required);
    }
    schema
}

/// The description the model reads: the author's prompt, plus the rendering
/// note when a view is actually linked.
fn tool_description(definition: &ToolDefinition) -> String {
    let mut description = definition.description.trim().to_string();
    if definition.genui_id.is_some() {
        description.push_str(RENDERED_SUFFIX);
    }
    description
}

/// Build the [`AgentTool`] for one definition.
///
/// The handler is a pure acknowledgement — see the module docs for why
/// execution carries no content. Argument validation is the agent loop's job
/// (it compiles [`params_to_json_schema`]'s output), so re-checking here would
/// only duplicate it.
pub fn make_dynamic_tool(definition: &ToolDefinition) -> AgentTool {
    let ack = if definition.genui_id.is_some() {
        RENDERED_ACK
    } else {
        NOT_RENDERED_ACK
    };

    AgentTool::simple(
        definition.name.clone(),
        tool_description(definition),
        params_to_json_schema(&definition.parameters),
        definition.display_name.clone(),
        move |_tool_call_id, _args| async move { ToolResult::text(ack) },
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{get_text, tokio_test_block};
    use super::*;

    fn param(name: &str, param_type: ToolParamType, required: bool) -> ToolParam {
        ToolParam {
            name: name.to_string(),
            param_type,
            description: String::new(),
            required,
        }
    }

    fn definition(parameters: Vec<ToolParam>, genui_id: Option<&str>) -> ToolDefinition {
        ToolDefinition {
            id: "tool-1".to_string(),
            name: "weather_card".to_string(),
            display_name: "Weather card".to_string(),
            icon: Some("cloud-sun".to_string()),
            description: "  Call when the user asks about the weather  ".to_string(),
            parameters,
            genui_id: genui_id.map(str::to_string),
            enabled: true,
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn every_parameter_type_compiles_to_a_concrete_schema() {
        let schema = params_to_json_schema(&[
            param("s", ToolParamType::String, false),
            param("n", ToolParamType::Number, false),
            param("b", ToolParamType::Boolean, false),
            param("list", ToolParamType::StringList, false),
            param("matrix", ToolParamType::StringMatrix, false),
            param("pairs", ToolParamType::KeyValueList, false),
        ]);

        let props = &schema["properties"];
        assert_eq!(props["s"]["type"], "string");
        assert_eq!(props["n"]["type"], "number");
        assert_eq!(props["b"]["type"], "boolean");
        assert_eq!(props["list"]["items"]["type"], "string");
        assert_eq!(props["matrix"]["items"]["items"]["type"], "string");
        assert_eq!(
            props["pairs"]["items"]["properties"]["key"]["type"],
            "string"
        );
        assert_eq!(props["pairs"]["items"]["required"], json!(["key", "value"]));
    }

    #[test]
    fn only_required_parameters_are_listed_as_required() {
        let schema = params_to_json_schema(&[
            param("city", ToolParamType::String, true),
            param("temp", ToolParamType::Number, false),
        ]);
        assert_eq!(schema["required"], json!(["city"]));
    }

    /// An empty `required` array is legal JSON Schema but some providers reject
    /// it, so it must be absent rather than empty.
    #[test]
    fn no_required_parameters_omits_the_key_entirely() {
        let schema = params_to_json_schema(&[param("city", ToolParamType::String, false)]);
        assert!(schema.get("required").is_none());
    }

    #[test]
    fn a_tool_with_no_parameters_still_has_an_object_schema() {
        let schema = params_to_json_schema(&[]);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"], json!({}));
        assert!(schema.get("required").is_none());
    }

    /// A blank description must not become `"description": ""`, which reads to
    /// the model as an empty instruction rather than none.
    #[test]
    fn a_blank_parameter_description_is_omitted() {
        let schema = params_to_json_schema(&[ToolParam {
            description: "   ".to_string(),
            ..param("city", ToolParamType::String, true)
        }]);
        assert!(schema["properties"]["city"].get("description").is_none());
    }

    #[test]
    fn a_parameter_description_reaches_the_schema_trimmed() {
        let schema = params_to_json_schema(&[ToolParam {
            description: "  City name  ".to_string(),
            ..param("city", ToolParamType::String, true)
        }]);
        assert_eq!(schema["properties"]["city"]["description"], "City name");
    }

    /// The rendering note is what stops the model narrating the card again, so
    /// it must ride along exactly when a view is linked.
    #[test]
    fn the_rendering_note_is_appended_only_with_a_linked_view() {
        let linked = make_dynamic_tool(&definition(Vec::new(), Some("genui-1")));
        assert!(linked.description.starts_with("Call when the user asks"));
        assert!(linked.description.contains("do not repeat its content"));

        let bare = make_dynamic_tool(&definition(Vec::new(), None));
        assert!(!bare.description.contains("do not repeat its content"));
    }

    #[test]
    fn the_tool_registers_under_its_name_and_label() {
        let tool = make_dynamic_tool(&definition(
            vec![param("city", ToolParamType::String, true)],
            Some("genui-1"),
        ));
        assert_eq!(tool.name, "weather_card");
        assert_eq!(tool.label, "Weather card");
        assert_eq!(tool.parameters["required"], json!(["city"]));
    }

    fn execute_ctx() -> hand_agent::ToolExecuteCtx {
        hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-dyn".to_string(),
            args: json!({ "city": "Hangzhou" }),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        }
    }

    /// A definition with no view must not tell the model it showed something.
    #[test]
    fn the_acknowledgement_states_whether_anything_was_rendered() {
        let linked = make_dynamic_tool(&definition(Vec::new(), Some("genui-1")));
        let result = tokio_test_block((linked.execute)(execute_ctx())).expect("execute ok");
        assert_eq!(get_text(&result), RENDERED_ACK);

        let bare = make_dynamic_tool(&definition(Vec::new(), None));
        let result = tokio_test_block((bare.execute)(execute_ctx())).expect("execute ok");
        assert!(get_text(&result).contains("no view linked"));
    }
}
