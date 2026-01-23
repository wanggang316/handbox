// 辅助功能权限命令

use tauri::command;

#[derive(Debug, serde::Serialize)]
pub struct AccessibilityError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AccessibilityError {}

#[cfg(target_os = "macos")]
mod macos_accessibility {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    // 注意：这里使用的是你清单里的 sys 库
    use accessibility_sys::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};

    // pub fn check_and_prompt() -> bool {
    //     unsafe {
    //         // 1. 获取系统定义的 Key 原始指针并包装成 CFString
    //         // wrap_under_get_rule 适用于系统常量，因为它不需要我们负责释放内存
    //         let prompt_key: CFString = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            
    //         // 2. 获取布尔值真
    //         let bool_true = CFBoolean::true_value();

    //         // 3. 构建字典 { "AXTrustedCheckOptionPrompt": true }
    //         // 注意：.as_CFType() 将具体类型转为通用的 CFType
    //         let options = CFDictionary::from_CFType_pairs(&[
    //             (prompt_key.as_CFType(), bool_true.as_CFType()),
    //         ]);

    //         // 4. 调用 API
    //         // options.as_concrete_TypeRef() 返回 AXIsProcessTrustedWithOptions 需要的字典引用
    //         AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    //     }
    // }

    pub fn check_and_prompt(prompt: bool) -> bool {
        unsafe {
            // 使用 accessibility-sys 提供的 kAXTrustedCheckOptionPrompt 常量
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let value = if prompt {
                CFBoolean::true_value()
            } else {
                CFBoolean::false_value()
            };
            let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
            let result = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
            tracing::info!(
                "is_trusted_with_prompt(prompt={}): result = {}",
                prompt,
                result
            );
            result
        }
    }

    pub fn open_settings() {
        // 如果系统弹窗被用户关了，我们可以提供一个手动打开设置页面的命令
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

/// 检查辅助功能权限是否已授予（静默检查，不显示系统弹窗）
#[command]
pub async fn accessibility_check_permission() -> Result<bool, AccessibilityError> {
    #[cfg(target_os = "macos")]
    {
        let result = macos_accessibility::check_and_prompt(true);
        tracing::info!("accessibility_check_permission: {}", result);
        Ok(result)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // 非 macOS 平台默认返回 true
        Ok(true)
    }
}

/// 请求辅助功能权限（如未授予则显示系统弹窗引导用户开启）
#[command]
pub async fn accessibility_request_permission() -> Result<bool, AccessibilityError> {
    #[cfg(target_os = "macos")]
    {
        tracing::info!("accessibility_request_permission: calling is_trusted_with_prompt(true)");
        // is_trusted_with_prompt(true) 会在未授权时自动弹出 macOS 标准的"辅助功能"授权提示窗
        let result = macos_accessibility::check_and_prompt(true);
        tracing::info!("accessibility_request_permission: result = {}", result);
        Ok(result)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// 打开系统辅助功能设置页面
#[command]
pub async fn accessibility_open_settings() -> Result<(), AccessibilityError> {
    #[cfg(target_os = "macos")]
    {
        macos_accessibility::open_settings();
    }
    Ok(())
}
