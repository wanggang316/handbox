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
    use accessibility_sys::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};

    pub fn check_and_prompt(prompt: bool) -> bool {
        unsafe {
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
        // Manual fallback for when the user has dismissed the system prompt
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

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
        Ok(true)
    }
}

/// Shows the standard macOS accessibility permission dialog when not yet granted.
#[command]
pub async fn accessibility_request_permission() -> Result<bool, AccessibilityError> {
    #[cfg(target_os = "macos")]
    {
        tracing::info!("accessibility_request_permission: calling is_trusted_with_prompt(true)");
        let result = macos_accessibility::check_and_prompt(true);
        tracing::info!("accessibility_request_permission: result = {}", result);
        Ok(result)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

#[command]
pub async fn accessibility_open_settings() -> Result<(), AccessibilityError> {
    #[cfg(target_os = "macos")]
    {
        macos_accessibility::open_settings();
    }
    Ok(())
}
