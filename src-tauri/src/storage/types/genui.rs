use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// GenUI 实体 - 一份具名、可复用的 JSON-Render UI spec。
///
/// `spec` 是原样的 spec JSON 文本（前端经 `explainSpec` 校验后保存），后端视其为
/// 不透明字符串、从不解析其内部结构。Chat agent 可通过 `agents.genui_id` 关联到它。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenUi {
    pub id: UUID,
    pub name: String,
    pub spec: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 GenUI 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGenUiRequest {
    pub name: String,
    pub spec: String,
}

/// 更新 GenUI 请求（按需更新名称 / spec）
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

    /// 锁定 JS<->Rust 线缆键：serde camelCase 把 `created_at` 转成 `createdAt`，
    /// 单词字段（id/name/spec）保持原样。前端类型必须与之匹配。
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
