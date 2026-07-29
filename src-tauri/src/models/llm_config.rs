use serde::{Deserialize, Serialize};

/// Provider configuration option exposed to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: String,
    pub type_name: String,
    pub default_name: String,
    pub default_base_url: String,
    pub icon: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigsResponse {
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
}
