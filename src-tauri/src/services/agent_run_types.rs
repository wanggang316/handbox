//! agent_run_types — wire types shared by the Agent-mode run path.
//!
//! These are the deserialization shapes for the `agent_run_stream` IPC command
//! and its image attachments. They were extracted from the now-retired legacy
//! `agent_runtime` driver so the coding-agent driver (`coding_agent_runtime`)
//! and the command layer (`commands::agent_run`) can keep consuming the exact
//! same `{ sessionId, input, attachments, forcedSkills }` payload the frontend
//! already sends — the platform shift to the coding-agent engine does not change
//! the wire contract.

use crate::storage::types::UUID;

/// 一个随本回合输入一并发送的图片附件（镜像 chat 的 `MessageRequestAttachment`）。
///
/// 前端已把文件读成原始字节并按 `image/*` 过滤；后端在装配 user 消息时把每个
/// 图片字节做 base64 STANDARD 编码，emit 成一个 `model::ImageContent` 块。非图片
/// mime 在装配处被防御性跳过（belt-and-suspenders）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// `agent_run_stream` 的入参。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    pub session_id: UUID,
    pub input: String,
    /// 可选的图片附件。缺省（旧调用方 / 纯文本发送）时为空 Vec，走纯文本路径。
    #[serde(default)]
    pub attachments: Vec<AgentRunAttachment>,
    /// 本回合显式强制加载的 skill 名（wire 上 camelCase `forcedSkills`）。缺省
    /// （旧的三字段 payload / 无强制注入）时为空 Vec。每个名针对**当前有效集**
    /// （discovered-and-validated ∖ globally-disabled）解析；解析到的 skill body
    /// 被逐字注入装配期的 system_prompt。unknown / 未发现 / 校验失败 / 全局禁用 /
    /// 空串一律静默跳过（disabled 优先于 forced）；但强制一个 opt-in skill 仍注入
    /// （显式用户意图覆盖 opt-in 抑制）。
    #[serde(default)]
    pub forced_skills: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VAL-SLASH-019: an old `{ sessionId, input, attachments }` payload (no
    /// `forcedSkills`) deserializes into `AgentRunRequest` with `forced_skills`
    /// defaulting to an empty Vec.
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

    /// An `image/*` attachment carrying raw bytes deserializes with its camelCase
    /// `mimeType` and base64-decoded `data` intact (the boundary types must
    /// faithfully reflect the frontend's `{ name, mimeType, data }` shape).
    #[test]
    fn attachment_deserializes_camel_case_fields() {
        let json = r#"{ "name": "shot.png", "mimeType": "image/png", "data": [1, 2, 3] }"#;
        let att: AgentRunAttachment = serde_json::from_str(json).expect("attachment deserializes");
        assert_eq!(att.name, "shot.png");
        assert_eq!(att.mime_type, "image/png");
        assert_eq!(att.data, vec![1u8, 2, 3]);
    }
}
