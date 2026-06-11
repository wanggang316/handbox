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

    /// Set a skill's global disabled flag in `skills.disabled` via a whole-file
    /// read-modify-write.
    ///
    /// `disabled = true` appends the name unless an equal entry already exists
    /// (dedup on insert); `disabled = false` removes every equal entry. All
    /// other list entries — orphans, duplicates, whitespace — are opaque
    /// storage and stay verbatim, and every other settings section round-trips
    /// untouched through the same load/save path as any settings update.
    pub fn set_skill_disabled(&self, name: &str, disabled: bool) -> Result<AppSettings, AppError> {
        let mut settings = self.load_or_default()?;
        if disabled {
            if !settings.skills.disabled.iter().any(|entry| entry == name) {
                settings.skills.disabled.push(name.to_string());
            }
        } else {
            settings.skills.disabled.retain(|entry| entry != name);
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

    /// Snapshot the four legacy sections as serde values for
    /// "other sections untouched" assertions (value equivalence after a
    /// serialization round-trip, per the validation contract).
    fn legacy_sections(settings: &AppSettings) -> [(&'static str, serde_json::Value); 4] {
        [
            ("general", serde_json::to_value(&settings.general).unwrap()),
            ("mcp", serde_json::to_value(&settings.mcp).unwrap()),
            ("account", serde_json::to_value(&settings.account).unwrap()),
            (
                "translation",
                serde_json::to_value(&settings.translation).unwrap(),
            ),
        ]
    }

    // VAL-CONFIG-003: disabling a skill writes its name into the config.json
    // `skills.disabled` array while every other section stays value-identical.
    #[test]
    fn set_skill_disabled_true_writes_name_and_preserves_other_sections() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);

        // Seed non-default values so "preserved" is distinguishable from
        // "reset to default".
        svc.update_settings(UpdateSettingsRequest {
            section: "general".to_string(),
            data: serde_json::json!({ "autoScroll": false, "theme": "dark" }),
        })
        .unwrap();
        let before = svc.get_settings().unwrap();
        let snapshot = legacy_sections(&before);

        let updated = svc.set_skill_disabled("alpha", true).unwrap();
        assert_eq!(updated.skills.disabled, vec!["alpha"]);

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path(&dir)).unwrap()).unwrap();
        assert_eq!(written["skills"]["disabled"], serde_json::json!(["alpha"]));
        for (key, expected) in snapshot {
            assert_eq!(written[key], expected, "section `{key}` must be untouched");
        }
    }

    // VAL-CONFIG-004: re-enabling removes the name (all equal entries) from
    // the persisted list.
    #[test]
    fn set_skill_disabled_false_removes_all_equal_entries() {
        let dir = TempDir::new().unwrap();
        // Seed a list that already contains duplicates of the target plus an
        // unrelated opaque entry.
        let mut value = serde_json::to_value(default_settings()).unwrap();
        value["skills"]["disabled"] = serde_json::json!(["alpha", "ghost", "alpha"]);
        fs::write(
            config_path(&dir),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let svc = service(&dir);
        let updated = svc.set_skill_disabled("alpha", false).unwrap();
        assert_eq!(updated.skills.disabled, vec!["ghost"]);

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path(&dir)).unwrap()).unwrap();
        assert_eq!(written["skills"]["disabled"], serde_json::json!(["ghost"]));
    }

    // Re-enabling a name that is not in the list is a no-op, not an error.
    #[test]
    fn set_skill_disabled_false_on_absent_name_is_noop() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let updated = svc.set_skill_disabled("never-disabled", false).unwrap();
        assert!(updated.skills.disabled.is_empty());
    }

    // Insert is deduplicating: disabling an already-disabled skill must not
    // grow the list.
    #[test]
    fn set_skill_disabled_true_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        svc.set_skill_disabled("alpha", true).unwrap();
        let updated = svc.set_skill_disabled("alpha", true).unwrap();
        assert_eq!(updated.skills.disabled, vec!["alpha"]);
    }

    // VAL-CONFIG-006 / VAL-CONFIG-007: back-to-back disables of distinct
    // skills read-modify-write the list — no lost update, both names persist.
    #[test]
    fn back_to_back_disables_keep_both_names() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        svc.set_skill_disabled("alpha", true).unwrap();
        let updated = svc.set_skill_disabled("beta", true).unwrap();
        assert_eq!(updated.skills.disabled, vec!["alpha", "beta"]);

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path(&dir)).unwrap()).unwrap();
        assert_eq!(
            written["skills"]["disabled"],
            serde_json::json!(["alpha", "beta"])
        );
    }

    // VAL-CONFIG-005: the list survives a "restart" — a fresh service over
    // the same data dir (config.json is the durable store) reads it back.
    #[test]
    fn disabled_list_survives_service_restart() {
        let dir = TempDir::new().unwrap();
        service(&dir).set_skill_disabled("alpha", true).unwrap();

        let reread = service(&dir).get_settings().unwrap();
        assert_eq!(reread.skills.disabled, vec!["alpha"]);
    }

    // Pre-existing opaque entries (orphans, duplicates, whitespace) stay
    // verbatim when an unrelated name is toggled.
    #[test]
    fn set_skill_disabled_preserves_opaque_entries_verbatim() {
        let dir = TempDir::new().unwrap();
        let mut value = serde_json::to_value(default_settings()).unwrap();
        value["skills"]["disabled"] = serde_json::json!(["ghost", "ghost", "", "  "]);
        fs::write(
            config_path(&dir),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let updated = service(&dir).set_skill_disabled("alpha", true).unwrap();
        assert_eq!(
            updated.skills.disabled,
            vec!["ghost", "ghost", "", "  ", "alpha"]
        );
    }

    // VAL-CONFIG-018: a disk-write failure surfaces as a structured
    // INTERNAL_ERROR — no panic. (fs::write is non-atomic; this asserts error
    // reporting only, not absence of partial writes.)
    #[cfg(unix)]
    #[test]
    fn set_skill_disabled_write_failure_returns_structured_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        // Materialize a valid config first, then make it unwritable.
        svc.get_settings().unwrap();
        fs::set_permissions(config_path(&dir), fs::Permissions::from_mode(0o444)).unwrap();

        let err = svc.set_skill_disabled("alpha", true).unwrap_err();
        assert_eq!(error_code(&err), "INTERNAL_ERROR");
        let json = serde_json::to_value(&err).unwrap();
        assert!(json["message"].is_string());
        assert!(json.get("hint").is_some());

        // Restore permissions so TempDir cleanup succeeds.
        fs::set_permissions(config_path(&dir), fs::Permissions::from_mode(0o644)).unwrap();
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
