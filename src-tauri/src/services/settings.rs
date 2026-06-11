// 设置服务实现

use crate::models::{
    AccountSettings, AppError, AppSettings, GeneralSettings, Language, MCPSettings,
    QuickToolsSettings, ShortcutConfig, SkillSettings, Theme, ThemeColor, TranslationSettings,
    UpdateSettingsRequest,
};
use crate::services::StorageService;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct SettingsService {
    storage: Arc<StorageService>,
}

impl SettingsService {
    pub fn new(storage: Arc<StorageService>) -> Self {
        Self { storage }
    }

    pub fn get_settings(&self) -> Result<AppSettings, AppError> {
        self.load_or_default()
    }

    pub fn update_settings(&self, request: UpdateSettingsRequest) -> Result<AppSettings, AppError> {
        let mut settings = self.load_or_default()?;
        let section = request.section.as_str();

        match section {
            "general" => {
                settings.general = self.merge_section(settings.general, request.data, "general")?;
            }
            "mcp" => {
                settings.mcp = self.merge_section(settings.mcp, request.data, "mcp")?;
            }
            "account" => {
                settings.account = self.merge_section(settings.account, request.data, "account")?;
            }
            "translation" => {
                settings.translation =
                    self.merge_section(settings.translation, request.data, "translation")?;
            }
            "quickTools" => {
                settings.quick_tools =
                    self.merge_section(settings.quick_tools, request.data, "quickTools")?;
            }
            "skills" => {
                settings.skills = self.merge_section(settings.skills, request.data, "skills")?;
            }
            _ => {
                return Err(AppError::validation_error("未知设置分组"));
            }
        }

        self.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn reset_settings(&self, sections: Option<Vec<String>>) -> Result<AppSettings, AppError> {
        let default_settings = default_settings();
        let settings = match sections {
            None => default_settings,
            Some(section_list) => {
                let mut current = self.load_or_default()?;
                for section in section_list {
                    match section.as_str() {
                        "general" => current.general = default_settings.general.clone(),
                        "mcp" => current.mcp = default_settings.mcp.clone(),
                        "account" => current.account = default_settings.account.clone(),
                        "translation" => current.translation = default_settings.translation.clone(),
                        "quickTools" => current.quick_tools = default_settings.quick_tools.clone(),
                        "skills" => current.skills = default_settings.skills.clone(),
                        _ => return Err(AppError::validation_error("未知设置分组")),
                    }
                }
                current
            }
        };

        self.save_settings(&settings)?;
        Ok(settings)
    }

    fn load_or_default(&self) -> Result<AppSettings, AppError> {
        let path = self.storage.get_config_path();
        if !path.exists() {
            let settings = default_settings();
            self.save_settings(&settings)?;
            return Ok(settings);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| AppError::internal_error(&format!("读取设置失败: {e}")))?;
        let settings: AppSettings = serde_json::from_str(&content)
            .map_err(|e| AppError::internal_error(&format!("解析设置失败: {e}")))?;

        Ok(settings)
    }

    fn save_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        let path = self.storage.get_config_path();
        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| AppError::internal_error(&format!("序列化设置失败: {e}")))?;
        std::fs::write(&path, content)
            .map_err(|e| AppError::internal_error(&format!("写入设置失败: {e}")))?;
        Ok(())
    }

    fn merge_section<T: serde::Serialize + serde::de::DeserializeOwned>(
        &self,
        section: T,
        patch: Value,
        section_name: &str,
    ) -> Result<T, AppError> {
        let mut target = serde_json::to_value(section).map_err(|e| {
            AppError::internal_error(&format!("序列化设置失败: {section_name}: {e}"))
        })?;

        merge_json(&mut target, &patch);

        serde_json::from_value(target).map_err(|e| {
            AppError::internal_error(&format!("反序列化设置失败: {section_name}: {e}"))
        })
    }
}

fn merge_json(target: &mut Value, patch: &Value) {
    if let (Value::Object(target_map), Value::Object(patch_map)) = (target, patch) {
        for (key, value) in patch_map {
            target_map.insert(key.clone(), value.clone());
        }
    }
}

fn default_settings() -> AppSettings {
    AppSettings {
        general: GeneralSettings {
            theme: Theme::System,
            theme_color: ThemeColor::System,
            language: Language::ZhCN,
            auto_scroll: true,
            shortcuts: ShortcutConfig {
                send_message: "Enter".to_string(),
                new_line: "Shift+Enter".to_string(),
                switch_model: None,
            },
        },
        mcp: MCPSettings {
            servers: Vec::new(),
        },
        account: AccountSettings {
            user: None,
            is_logged_in: false,
        },
        translation: TranslationSettings { session_id: None },
        quick_tools: QuickToolsSettings {
            show_toolbar_on_selection: false,
            selection_blacklist: Default::default(),
        },
        skills: SkillSettings::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn service(dir: &TempDir) -> SettingsService {
        let storage = StorageService::new(dir.path().to_path_buf()).unwrap();
        SettingsService::new(Arc::new(storage))
    }

    fn config_path(dir: &TempDir) -> PathBuf {
        dir.path().join("config.json")
    }

    /// A valid pre-revamp config: the four legacy sections present, no
    /// `skills` section at all.
    fn config_without_skills_section() -> String {
        let mut value = serde_json::to_value(default_settings()).unwrap();
        let map = value.as_object_mut().unwrap();
        map.remove("skills");
        assert!(map.contains_key("general"), "fixture keeps legacy sections");
        assert!(map.contains_key("mcp"));
        assert!(map.contains_key("account"));
        assert!(map.contains_key("translation"));
        serde_json::to_string_pretty(&value).unwrap()
    }

    fn error_code(err: &AppError) -> String {
        serde_json::to_value(err).unwrap()["code"]
            .as_str()
            .unwrap()
            .to_string()
    }

    // VAL-CONFIG-001 (storage half): fresh environment, no settings ever
    // written → skills.disabled defaults to empty, and the auto-written
    // config.json carries an empty (or absent) skills.disabled.
    #[test]
    fn fresh_env_defaults_to_empty_disabled_list() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);

        let settings = svc.get_settings().unwrap();
        assert!(settings.skills.disabled.is_empty());

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path(&dir)).unwrap()).unwrap();
        let disabled = written.pointer("/skills/disabled");
        assert!(
            disabled.is_none() || disabled == Some(&serde_json::json!([])),
            "default skills.disabled must be empty or absent: {disabled:?}"
        );
    }

    // VAL-CONFIG-013: a valid config.json missing the `skills` section parses
    // without error via serde(default) → all enabled.
    #[test]
    fn missing_skills_section_parses_with_all_enabled() {
        let dir = TempDir::new().unwrap();
        fs::write(config_path(&dir), config_without_skills_section()).unwrap();

        let settings = service(&dir).get_settings().unwrap();
        assert!(settings.skills.disabled.is_empty());
    }

    // VAL-CONFIG-008 / VAL-CONFIG-011 (storage half): the disabled list is
    // opaque storage — orphan, duplicate, empty and whitespace entries persist
    // verbatim across an unrelated settings update (no prune, no
    // normalization, no dedup).
    #[test]
    fn disabled_entries_persist_verbatim_across_unrelated_update() {
        let dir = TempDir::new().unwrap();
        let entries = serde_json::json!(["ghost", "ghost", "", "  ", "MySkill"]);
        let mut value = serde_json::to_value(default_settings()).unwrap();
        value["skills"]["disabled"] = entries.clone();
        fs::write(
            config_path(&dir),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let svc = service(&dir);
        let settings = svc.get_settings().unwrap();
        assert_eq!(
            settings.skills.disabled,
            vec!["ghost", "ghost", "", "  ", "MySkill"]
        );

        // Unrelated update re-saves the file; the list must survive verbatim.
        svc.update_settings(UpdateSettingsRequest {
            section: "general".to_string(),
            data: serde_json::json!({ "autoScroll": false }),
        })
        .unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path(&dir)).unwrap()).unwrap();
        assert_eq!(written["skills"]["disabled"], entries);
    }

    // VAL-CONFIG-017: corrupt (non-JSON) config.json → structured
    // INTERNAL_ERROR, file not silently overwritten.
    #[test]
    fn corrupt_config_returns_internal_error_and_keeps_file() {
        let dir = TempDir::new().unwrap();
        let corrupt = "not json {{{";
        fs::write(config_path(&dir), corrupt).unwrap();

        let err = service(&dir).get_settings().unwrap_err();
        assert_eq!(error_code(&err), "INTERNAL_ERROR");
        assert_eq!(
            fs::read_to_string(config_path(&dir)).unwrap(),
            corrupt,
            "corrupt file must not be overwritten"
        );
    }

    // VAL-CONFIG-016: structurally illegal skills.disabled (non-array, or a
    // non-string element) → structured INTERNAL_ERROR, file untouched.
    #[test]
    fn illegal_disabled_shape_returns_internal_error() {
        for bad in [
            serde_json::json!("nope"),
            serde_json::json!(["ok", 42]),
            serde_json::json!({ "k": "v" }),
        ] {
            let dir = TempDir::new().unwrap();
            let mut value = serde_json::to_value(default_settings()).unwrap();
            value["skills"]["disabled"] = bad;
            let content = serde_json::to_string_pretty(&value).unwrap();
            fs::write(config_path(&dir), &content).unwrap();

            let err = service(&dir).get_settings().unwrap_err();
            assert_eq!(error_code(&err), "INTERNAL_ERROR");
            assert_eq!(
                fs::read_to_string(config_path(&dir)).unwrap(),
                content,
                "file must stay untouched on a structural error"
            );
        }
    }

    // The closed section enum recognizes "skills" in both update and reset.
    #[test]
    fn update_and_reset_recognize_skills_section() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);

        let updated = svc
            .update_settings(UpdateSettingsRequest {
                section: "skills".to_string(),
                data: serde_json::json!({ "disabled": ["alpha"] }),
            })
            .unwrap();
        assert_eq!(updated.skills.disabled, vec!["alpha"]);

        let reread = svc.get_settings().unwrap();
        assert_eq!(reread.skills.disabled, vec!["alpha"]);

        let reset = svc
            .reset_settings(Some(vec!["skills".to_string()]))
            .unwrap();
        assert!(reset.skills.disabled.is_empty());
    }
}
