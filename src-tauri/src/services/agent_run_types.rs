//! agent_run_types — wire types shared by the Agent-mode run path.
//!
//! Deserialization shapes for the `agent_run_stream` IPC command and its image
//! attachments. Kept in one place so the coding-agent driver
//! (`coding_agent_runtime`) and the command layer (`commands::agent_run`) share
//! a single `{ sessionId, input, attachments, forcedSkills }` wire contract.

use crate::storage::types::UUID;

/// An image attachment sent alongside the turn input (mirrors chat's
/// `MessageRequestAttachment`). Bytes arrive raw and pre-filtered to `image/*`;
/// assembly base64-encodes each into a `model::ImageContent` block and
/// defensively skips non-image mimes.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Input payload of `agent_run_stream`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    pub session_id: UUID,
    pub input: String,
    /// Optional image attachments; empty means the plain-text path.
    #[serde(default)]
    pub attachments: Vec<AgentRunAttachment>,
    /// Skill names force-loaded for this turn. Each is resolved against the
    /// currently effective set (discovered-and-validated minus globally
    /// disabled) and its body injected verbatim into the assembled
    /// system_prompt. Unknown / invalid / globally-disabled / empty names are
    /// skipped silently (disabled wins over forced), but forcing an opt-in
    /// skill does inject it — explicit user intent overrides opt-in gating.
    #[serde(default)]
    pub forced_skills: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `{ sessionId, input, attachments }` payload without `forcedSkills`
    /// still deserializes, with `forced_skills` defaulting to an empty Vec.
    #[test]
    fn legacy_payload_deserializes_with_empty_forced_skills() {
        // Three-field legacy payload.
        let json = r#"{ "sessionId": "s-1", "input": "hi", "attachments": [] }"#;
        let req: AgentRunRequest = serde_json::from_str(json).expect("legacy payload deserializes");
        assert_eq!(req.session_id, "s-1");
        assert_eq!(req.input, "hi");
        assert!(req.attachments.is_empty());
        assert!(
            req.forced_skills.is_empty(),
            "forced_skills defaults to empty for a legacy payload"
        );

        // Even the two-field payload (no attachments) defaults both.
        let minimal = r#"{ "sessionId": "s-2", "input": "yo" }"#;
        let req2: AgentRunRequest = serde_json::from_str(minimal).expect("minimal payload");
        assert!(req2.attachments.is_empty());
        assert!(req2.forced_skills.is_empty());

        // A payload WITH forcedSkills round-trips into the field.
        let with = r#"{ "sessionId": "s-3", "input": "go", "forcedSkills": ["alpha", "beta"] }"#;
        let req3: AgentRunRequest = serde_json::from_str(with).expect("forced payload");
        assert_eq!(
            req3.forced_skills,
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    /// The boundary type must faithfully reflect the frontend's
    /// `{ name, mimeType, data }` shape, camelCase included.
    #[test]
    fn attachment_deserializes_camel_case_fields() {
        let json = r#"{ "name": "shot.png", "mimeType": "image/png", "data": [1, 2, 3] }"#;
        let att: AgentRunAttachment = serde_json::from_str(json).expect("attachment deserializes");
        assert_eq!(att.name, "shot.png");
        assert_eq!(att.mime_type, "image/png");
        assert_eq!(att.data, vec![1u8, 2, 3]);
    }
}
