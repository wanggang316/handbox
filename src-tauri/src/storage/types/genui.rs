use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// A named, reusable JSON-Render UI spec.
///
/// `spec` is the raw spec JSON text (validated by the frontend via
/// `explainSpec`); the backend treats it as an opaque string and never parses
/// it. Chat agents can reference it through `agents.genui_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenUi {
    pub id: UUID,
    pub name: String,
    pub spec: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGenUiRequest {
    pub name: String,
    pub spec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGenUiRequest {
    pub name: Option<String>,
    pub spec: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genui_serialization_roundtrip() {
        let genui = GenUi {
            id: "genui_1".to_string(),
            name: "Translation Card".to_string(),
            spec: r#"{"root":"card","elements":{}}"#.to_string(),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&genui).expect("serialize");
        let deserialized: GenUi = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(genui.id, deserialized.id);
        assert_eq!(genui.name, deserialized.name);
        assert_eq!(genui.spec, deserialized.spec);
    }

    /// Locks the JS<->Rust wire keys: serde camelCase maps `created_at` to
    /// `createdAt`; single-word fields (id/name/spec) stay as-is. Frontend
    /// types must match.
    #[test]
    fn genui_wire_keys_are_camel_case() {
        let genui = GenUi {
            id: "genui_1".to_string(),
            name: "Card".to_string(),
            spec: "{}".to_string(),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&genui).expect("serialize");
        assert!(
            json.contains("\"createdAt\""),
            "expected camelCase createdAt: {json}"
        );
        assert!(
            json.contains("\"updatedAt\""),
            "expected camelCase updatedAt: {json}"
        );
        assert!(json.contains("\"spec\""), "expected spec key: {json}");
    }

    #[test]
    fn create_genui_request_deserializes() {
        let json = r#"{"name": "My UI", "spec": "{}"}"#;
        let req: CreateGenUiRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "My UI");
        assert_eq!(req.spec, "{}");
    }
}
