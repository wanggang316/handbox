//! Runtime refresh of hand-ai's model catalog.
//!
//! Keeps the in-memory catalog fresh without a dependency bump, so models
//! hand-ai adds after our pinned snapshot still resolve at chat time. Layering
//! is hand-ai's — `embedded baseline > local cache > remote` — and every step
//! degrades gracefully: on any error the in-memory catalog is left untouched.

use std::time::Duration;

/// Rolling Release asset hand-ai publishes the regenerated catalog to.
const CATALOG_URL: &str =
    "https://github.com/wanggang316/hand-ai/releases/download/catalog/models.json";

/// Matches hand-ai's daily regeneration cadence. An unchanged catalog costs a
/// single `304` via the cached ETag.
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Runs entirely in the background so catalog I/O never sits on the startup
/// critical path; later `get_model` / `get_models` reads pick up the fresher
/// catalog without a restart.
pub fn spawn() {
    tauri::async_runtime::spawn(async {
        // A prior run's catalog is active before the slower network refresh lands.
        prime_from_cache();

        loop {
            match hand_ai_model::refresh_from_remote(CATALOG_URL).await {
                Ok(hand_ai_model::RefreshOutcome::Updated { providers, models }) => {
                    tracing::info!(providers, models, "hand-ai catalog refreshed from remote");
                }
                Ok(hand_ai_model::RefreshOutcome::Unchanged) => {
                    tracing::debug!("hand-ai catalog unchanged (304)");
                }
                Err(e) => {
                    tracing::warn!("hand-ai catalog refresh failed (keeping current): {e}");
                }
            }
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
}

/// Loads the last-fetched catalog (`~/.hand-ai`) into the in-memory registry.
/// No-op when no cache exists: the embedded baseline stays active.
fn prime_from_cache() {
    if hand_ai_model::load_cached_catalog() {
        tracing::info!("Loaded hand-ai catalog from local cache (~/.hand-ai)");
    } else {
        tracing::debug!("No cached hand-ai catalog; using embedded baseline");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_url_points_at_hand_ai_release_asset() {
        // Pin the endpoint: a typo here silently disables runtime refresh
        // (every fetch 404s, the warn path swallows it, baseline stays).
        assert_eq!(
            CATALOG_URL,
            "https://github.com/wanggang316/hand-ai/releases/download/catalog/models.json"
        );
        assert!(CATALOG_URL.starts_with("https://"), "must be HTTPS");
    }

    #[test]
    fn refresh_interval_is_daily() {
        assert_eq!(REFRESH_INTERVAL, Duration::from_secs(86_400));
    }

    /// Proves the wired URL pulls a catalog where the embedded baseline's
    /// OpenRouter gap (e.g. the `~*-latest` aliases) is filled.
    #[tokio::test]
    #[ignore = "network: hits hand-ai's live Release asset + writes ~/.hand-ai"]
    async fn refresh_resolves_openrouter_latest_alias() {
        const ALIAS: &str = "~google/gemini-flash-latest";

        let outcome = hand_ai_model::refresh_from_remote(CATALOG_URL)
            .await
            .expect("refresh should succeed against the live release asset");
        match outcome {
            hand_ai_model::RefreshOutcome::Updated { providers, models } => {
                assert!(providers > 0 && models > 0, "non-empty catalog installed");
            }
            hand_ai_model::RefreshOutcome::Unchanged => { /* cache already current */ }
        }

        assert!(
            hand_ai_model::get_model("openrouter", ALIAS).is_some(),
            "OpenRouter alias {ALIAS} should resolve after catalog refresh"
        );
    }
}
