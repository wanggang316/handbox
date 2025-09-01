// 模型数据库 - 包含各个供应商的详细模型信息
use crate::models::provider::{ModelFeature};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub context_length: Option<i32>,
    pub input_cost: Option<f32>, // 每1K tokens的成本 USD
    pub output_cost: Option<f32>, // 每1K tokens的成本 USD
    pub supported_features: Vec<ModelFeature>,
}

/// OpenAI 模型信息数据库
pub fn get_openai_model_info(model_id: &str) -> Option<ModelInfo> {
    match model_id {
        // GPT-4 系列
        "gpt-4" => Some(ModelInfo {
            name: "GPT-4".to_string(),
            context_length: Some(8192),
            input_cost: Some(0.03),
            output_cost: Some(0.06),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "gpt-4-turbo" | "gpt-4-turbo-preview" => Some(ModelInfo {
            name: "GPT-4 Turbo".to_string(),
            context_length: Some(128000),
            input_cost: Some(0.01),
            output_cost: Some(0.03),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "gpt-4o" => Some(ModelInfo {
            name: "GPT-4o".to_string(),
            context_length: Some(128000),
            input_cost: Some(0.005),
            output_cost: Some(0.015),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "gpt-4o-mini" => Some(ModelInfo {
            name: "GPT-4o Mini".to_string(),
            context_length: Some(128000),
            input_cost: Some(0.00015),
            output_cost: Some(0.0006),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        // GPT-3.5 系列
        "gpt-3.5-turbo" => Some(ModelInfo {
            name: "GPT-3.5 Turbo".to_string(),
            context_length: Some(16385),
            input_cost: Some(0.0015),
            output_cost: Some(0.002),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "gpt-3.5-turbo-instruct" => Some(ModelInfo {
            name: "GPT-3.5 Turbo Instruct".to_string(),
            context_length: Some(4096),
            input_cost: Some(0.0015),
            output_cost: Some(0.002),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Streaming,
            ],
        }),
        // o1 系列
        "o1-preview" => Some(ModelInfo {
            name: "o1 Preview".to_string(),
            context_length: Some(128000),
            input_cost: Some(0.015),
            output_cost: Some(0.06),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Reasoning,
            ],
        }),
        "o1-mini" => Some(ModelInfo {
            name: "o1 Mini".to_string(),
            context_length: Some(128000),
            input_cost: Some(0.003),
            output_cost: Some(0.012),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Reasoning,
            ],
        }),
        _ => None,
    }
}

/// DeepSeek 模型信息数据库
pub fn get_deepseek_model_info(model_id: &str) -> Option<ModelInfo> {
    match model_id {
        "deepseek-chat" => Some(ModelInfo {
            name: "DeepSeek Chat".to_string(),
            context_length: Some(32768),
            input_cost: Some(0.00014),
            output_cost: Some(0.00028),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "deepseek-coder" => Some(ModelInfo {
            name: "DeepSeek Coder".to_string(),
            context_length: Some(16384),
            input_cost: Some(0.00014),
            output_cost: Some(0.00028),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "deepseek-reasoner" => Some(ModelInfo {
            name: "DeepSeek Reasoner".to_string(),
            context_length: Some(32768),
            input_cost: Some(0.00055),
            output_cost: Some(0.0019),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Reasoning,
                ModelFeature::Streaming,
            ],
        }),
        _ => None,
    }
}

/// Anthropic 模型信息数据库
pub fn get_anthropic_model_info(model_id: &str) -> Option<ModelInfo> {
    match model_id {
        "claude-3-opus-20240229" => Some(ModelInfo {
            name: "Claude 3 Opus".to_string(),
            context_length: Some(200000),
            input_cost: Some(0.015),
            output_cost: Some(0.075),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "claude-3-sonnet-20240229" => Some(ModelInfo {
            name: "Claude 3 Sonnet".to_string(),
            context_length: Some(200000),
            input_cost: Some(0.003),
            output_cost: Some(0.015),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "claude-3-haiku-20240307" => Some(ModelInfo {
            name: "Claude 3 Haiku".to_string(),
            context_length: Some(200000),
            input_cost: Some(0.00025),
            output_cost: Some(0.00125),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "claude-3-5-sonnet-20241022" => Some(ModelInfo {
            name: "Claude 3.5 Sonnet".to_string(),
            context_length: Some(200000),
            input_cost: Some(0.003),
            output_cost: Some(0.015),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        "claude-3-5-haiku-20241022" => Some(ModelInfo {
            name: "Claude 3.5 Haiku".to_string(),
            context_length: Some(200000),
            input_cost: Some(0.001),
            output_cost: Some(0.005),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
                ModelFeature::Streaming,
            ],
        }),
        _ => None,
    }
}

/// 解析定价字符串为浮点数（OpenRouter格式）
pub fn parse_openrouter_price(price_str: &str) -> Option<f32> {
    // OpenRouter的价格格式通常是 "0.000003" 或 "3e-06"
    price_str.parse::<f32>().ok().map(|p| p * 1000.0) // 转换为每1K tokens
}

/// 解析模态性，确定支持的功能
pub fn parse_openrouter_features(modality: Option<&str>, model_name: &str) -> Vec<ModelFeature> {
    let mut features = vec![ModelFeature::Text, ModelFeature::Streaming];
    
    // 根据模态性添加功能
    if let Some(modality) = modality {
        if modality.contains("image") {
            features.push(ModelFeature::Vision);
        }
    }
    
    // 大多数现代模型都支持函数调用
    if !model_name.to_lowercase().contains("instruct") && 
       !model_name.to_lowercase().contains("base") {
        features.push(ModelFeature::FunctionCalling);
    }
    
    // 检测推理模型
    if model_name.to_lowercase().contains("reasoning") || 
       model_name.to_lowercase().contains("think") ||
       model_name.to_lowercase().contains("o1") {
        features.push(ModelFeature::Reasoning);
    }
    
    features
}
