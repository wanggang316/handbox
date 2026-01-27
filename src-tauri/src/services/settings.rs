// 设置服务实现

use crate::models::{
    AccountSettings, AppError, AppSettings, GeneralSettings, Language, MCPSettings,
    QuickToolsSettings, ShortcutConfig, Theme, ThemeColor, TranslationSettings,
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
                        "quickTools" => {
                            current.quick_tools = default_settings.quick_tools.clone()
                        }
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
        translation: TranslationSettings {
            model_id: None,
            provider_id: None,
            target_language: "system".to_string(),
        },
        quick_tools: QuickToolsSettings {
            show_toolbar_on_selection: false,
            selection_blacklist: Default::default(),
        },
    }
}
