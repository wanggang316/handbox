use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};
use std::fmt;

/// `api_key` is sensitive: `Debug` is hand-written so `{:?}`/log output never
/// leaks the full key (only set-ness + last 4 chars). `Serialize` still emits
/// the full key — the frontend/IPC needs it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: UUID,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl fmt::Debug for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("provider_type", &self.provider_type)
            .field("base_url", &self.base_url)
            .field("api_key", &redact_secret(&self.api_key))
            .field("enabled", &self.enabled)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

/// Redacts a secret for printing: empty → `<unset>`, else `***` + last 4
/// chars (taken per char to avoid panics on non-ASCII boundaries).
fn redact_secret(key: &str) -> String {
    if key.is_empty() {
        return "<unset>".to_string();
    }
    let last4: String = {
        let mut chars: Vec<char> = key.chars().rev().take(4).collect();
        chars.reverse();
        chars.into_iter().collect()
    };
    format!("***{last4}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(api_key: &str) -> Provider {
        Provider {
            id: "id-1".to_string(),
            name: "OpenRouter".to_string(),
            provider_type: "openrouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key: api_key.to_string(),
            enabled: true,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn debug_redacts_full_api_key() {
        let p = sample("sk-or-v1-secrethere1234");
        let dbg = format!("{p:?}");
        assert!(
            !dbg.contains("sk-or-v1-secrethere"),
            "debug output leaked the key: {dbg}"
        );
        assert!(
            dbg.contains("***1234"),
            "expected last-4 marker, got: {dbg}"
        );
        // non-sensitive fields still visible for diagnostics
        assert!(dbg.contains("OpenRouter") && dbg.contains("openrouter"));
    }

    #[test]
    fn debug_handles_empty_and_short_keys() {
        assert!(format!("{:?}", sample("")).contains("<unset>"));
        // short key: still no full leak — only the (short) last-4 marker form
        let dbg = format!("{:?}", sample("ab"));
        assert!(dbg.contains("***ab"));
    }

    #[test]
    fn redact_secret_is_char_boundary_safe() {
        // multi-byte chars must not panic the last-4 slice
        assert_eq!(redact_secret("密钥很长很长"), "***很长很长");
        assert_eq!(redact_secret(""), "<unset>");
    }
}
