use crate::models::Model;

pub mod chat;
pub mod model;
pub mod types;

mod client;

pub use chat::ChatClient;
pub use client::{create_chat_client, create_client, create_llm_client, LlmClient};
pub use model::ModelClient;
pub use types::{
    ChatApiType, ChatChoice, ChatMessage, ChatRequest, ChatResponse, ChatUsage, ModelApiType,
    ModelFeature, StandardModel,
};

/// 适配器：将 StandardModel 转换为应用内部的 Model 结构
pub fn adapt_model(standard_model: types::StandardModel, provider_id: String, now: i64) -> Model {
    let supported_features = standard_model.supported_features.map(|features| {
        features
            .into_iter()
            .map(|f| match f {
                types::ModelFeature::Chat => crate::models::provider::ModelFeature::Text,
                types::ModelFeature::Vision => crate::models::provider::ModelFeature::Vision,
                types::ModelFeature::FunctionCalling => {
                    crate::models::provider::ModelFeature::FunctionCalling
                }
                types::ModelFeature::Completion => crate::models::provider::ModelFeature::Text,
                types::ModelFeature::Embedding => crate::models::provider::ModelFeature::Text,
                types::ModelFeature::Streaming => crate::models::provider::ModelFeature::Streaming,
            })
            .collect()
    });

    Model {
        id: standard_model.id,
        provider_id,
        name: standard_model.name,
        context_length: standard_model.context_length,
        input_cost: standard_model.input_cost,
        output_cost: standard_model.output_cost,
        supported_features,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}
