// Tauri IPC bridge over hand-ai's provider/model catalog.
//
// Behind the top-level `hand-ai` feature flag — disabled in shipped
// HandBox builds today, enabled during integration via
// `cargo tauri dev --features hand-ai`. The library bridge lives in
// `handbox-llm::hand_ai_catalog`; this file is only the IPC adaptor.

use crate::services::hand_ai_catalog::{self, HandAiProviderInfo};

use crate::models::AppError;

/// List every provider hand-ai knows about, with capabilities and
/// per-model metadata, for the frontend's provider-selection UI.
#[tauri::command]
pub async fn hand_ai_list_providers() -> Result<Vec<HandAiProviderInfo>, AppError> {
    Ok(hand_ai_catalog::list_providers())
}
