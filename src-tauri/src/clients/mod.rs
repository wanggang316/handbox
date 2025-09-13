// 供应商客户端模块
// 提供统一的接口来调用不同供应商的API

use crate::models::Model;

pub mod chat_client;
pub mod llm_client;
pub mod model_client;

// 重新导出 llm_client 中的公共接口
pub use llm_client::{create_client, create_llm_client, LlmClient, StandardModel};

/// 适配器：将StandardModel转换为我们的Model结构
pub fn adapt_model(standard_model: StandardModel, provider_id: String, now: i64) -> Model {
    Model {
        id: standard_model.id,
        provider_id,
        name: standard_model.name,
        context_length: standard_model.context_length,
        input_cost: standard_model.input_cost,
        output_cost: standard_model.output_cost,
        supported_features: standard_model.supported_features,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}
