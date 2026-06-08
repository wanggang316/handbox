//! Runtime refresh of hand-ai's model catalog.
//!
//! HandBox depends entirely on hand-ai's catalog for the model list (no local
//! synthesis, no HandBox-side `/v1/models` polling). The `hand-ai-model` crate
//! ships an embedded baseline snapshot compiled in at the pinned version, but
//! hand-ai also publishes a daily-regenerated catalog as a GitHub Release
//! asset. Wiring hand-ai's `catalog_refresh` API keeps HandBox's in-memory
//! catalog fresh **without a dependency bump** — so models hand-ai added
//! upstream after our pinned snapshot (e.g. OpenRouter's full tool-capable
//! list, including the `~*-latest` pointer aliases) resolve at chat time.
//!
//! The data source stays hand-ai; this only upgrades the catalog from "static
//! embedded" to "embedded baseline + runtime hot-swap". Layering (hand-ai's):
//! `embedded baseline > local cache > remote`, every step degrading
//! gracefully — on any error the in-memory catalog is left untouched.

use std::time::Duration;

/// Rolling Release asset hand-ai publishes the regenerated catalog to.
const CATALOG_URL: &str =
    "https://github.com/wanggang316/hand-ai/releases/download/catalog/models.json";

/// Re-fetch interval. hand-ai regenerates daily; matching that keeps the
/// catalog within a day of upstream without hammering the endpoint — an
/// unchanged catalog costs a single `304` via the cached ETag.
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Load the last-fetched catalog from the local cache (`~/.hand-ai`) into the
/// in-memory registry, so a fresh catalog from a previous run is used
/// immediately instead of waiting on the network. No-op (the embedded baseline
/// stays active) when no cache exists.
///
/// Call once at startup **before** the provider list is built so the provider
/// augmentation reflects the cached catalog.
pub fn prime_from_cache() {
    if hand_ai_model::load_cached_catalog() {
        tracing::info!("Loaded hand-ai catalog from local cache (~/.hand-ai)");
    } else {
        tracing::debug!("No cached hand-ai catalog; using embedded baseline");
    }
}

/// Spawn a background task that refreshes the catalog from hand-ai's Release
/// asset immediately and every [`REFRESH_INTERVAL`] thereafter.
///
/// Every step degrades gracefully: on any error the in-memory catalog is left
/// untouched and the previously installed data keeps serving. The first
/// successful refresh hot-swaps the in-memory registry, so subsequent
/// `get_model` / `get_models` reads (chat resolution, model sync) pick up
/// upstream additions without a restart.
pub fn spawn_refresh_loop() {
    tauri::async_runtime::spawn(async {
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
        // Matches hand-ai's daily regeneration cadence.
        assert_eq!(REFRESH_INTERVAL, Duration::from_secs(86_400));
    }

    /// Live verification (network + writes the real `~/.hand-ai` cache).
    /// Run explicitly:
    ///   cargo test --lib services::catalog_sync::tests::refresh_resolves \
    ///       -- --ignored --exact
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
