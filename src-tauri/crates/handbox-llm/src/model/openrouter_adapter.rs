// OpenRouter 模型客户端实现

use super::model_client::ModelClient;
use crate::error::LlmClientError;
use crate::types::{LlmModelFeature, LlmModelModality, LlmProvider, LlmStandardModel};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// OpenRouter 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModel>,
}

/// OpenRouter 完整模型定义
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModel {
    /// 模型唯一标识符
    pub id: String,

    /// 规范的模型别名
    #[serde(default)]
    pub canonical_slug: Option<String>,

    /// 模型显示名称
    pub name: String,

    /// 创建时间 (Unix 时间戳)
    #[serde(default)]
    pub created: Option<i64>,

    /// 模型描述
    #[serde(default)]
    pub description: Option<String>,

    /// 上下文窗口长度
    pub context_length: i32,

    /// 架构信息
    #[serde(default)]
    pub architecture: Option<OpenRouterArchitecture>,

    /// 价格信息
    pub pricing: OpenRouterPricing,

    /// 顶级提供商信息
    #[serde(default)]
    pub top_provider: Option<OpenRouterTopProvider>,

    /// 每个请求的限制
    #[serde(default)]
    pub per_request_limits: Option<Value>,

    /// 支持的参数列表
    #[serde(default)]
    pub supported_parameters: Option<Vec<OpenRouterParameter>>,

    /// 默认参数配置
    #[serde(default)]
    pub default_parameters: Option<OpenRouterDefaultParameters>,
}

/// OpenRouter 默认参数配置
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterDefaultParameters {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub repetition_penalty: Option<f64>,
    #[serde(default)]
    pub min_p: Option<f64>,
    #[serde(default)]
    pub top_a: Option<f64>,
}

/// OpenRouter 支持的参数类型
/// 参考: https://openrouter.ai/docs/overview/models#supported-parameters
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenRouterParameter {
    /// 工具调用支持
    Tools,
    /// 工具选择策略
    ToolChoice,

    /// 最大生成 token 数
    MaxTokens,

    /// 采样温度 (0.0-2.0)
    Temperature,
    /// Top-p 核采样
    TopP,

    /// 推理模式支持
    Reasoning,
    /// 在响应中包含推理过程
    IncludeReasoning,

    /// 结构化输出支持
    StructuredOutputs,
    /// 响应格式控制
    ResponseFormat,

    /// 停止序列
    Stop,

    /// 频率惩罚 (-2.0 to 2.0)
    FrequencyPenalty,
    /// 存在惩罚 (-2.0 to 2.0)
    PresencePenalty,

    /// 随机种子
    Seed,

    /// 其他未知参数
    #[serde(other)]
    Unknown,
}

/// OpenRouter 模型架构信息
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterArchitecture {
    /// 输入模态
    #[serde(default)]
    pub input_modalities: Option<Vec<String>>,

    /// 输出模态
    #[serde(default)]
    pub output_modalities: Option<Vec<String>>,

    /// 分词器类型
    #[serde(default)]
    pub tokenizer: Option<String>,

    /// 指令类型
    #[serde(default)]
    pub instruct_type: Option<String>,
}

/// OpenRouter 价格信息
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct OpenRouterPricing {
    /// 提示词价格 (每百万 token，API 返回字符串格式如 "0.00001")
    #[serde(deserialize_with = "deserialize_price")]
    pub prompt: f32,

    /// 补全价格 (每百万 token，API 返回字符串格式如 "0.00002")
    #[serde(deserialize_with = "deserialize_price")]
    pub completion: f32,

    /// 每次请求价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub request: Option<f32>,

    /// 图片价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub image: Option<f32>,

    /// 网络搜索价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub web_search: Option<f32>,

    /// 内部推理价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub internal_reasoning: Option<f32>,

    /// 输入缓存读取价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub input_cache_read: Option<f32>,

    /// 输入缓存写入价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub input_cache_write: Option<f32>,
}

/// OpenRouter 顶级提供商信息
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterTopProvider {
    /// 上下文长度
    #[serde(default)]
    pub context_length: Option<i32>,

    /// 最大补全 token 数
    #[serde(default)]
    pub max_completion_tokens: Option<i32>,

    /// 是否经过审核
    #[serde(default)]
    pub is_moderated: Option<bool>,
}

/// 价格字符串反序列化为 f32
/// OpenRouter API 返回价格为字符串格式（如 "0.00001"），需要转换为数字
fn deserialize_price<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f32>().map_err(D::Error::custom)
}

/// 可选价格字符串反序列化为 Option<f32>
/// 支持 null、缺失字段或空字符串
fn deserialize_optional_price<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.is_empty() => s.parse::<f32>().map(Some).map_err(D::Error::custom),
        _ => Ok(None),
    }
}

impl OpenRouterModel {
    /// 转换为标准模型格式
    pub fn to_standard_model(self) -> LlmStandardModel {
        let output_token_limit = self
            .top_provider
            .as_ref()
            .and_then(|p| p.max_completion_tokens);

        let input_modalities = self
            .architecture
            .as_ref()
            .and_then(|arch| arch.input_modalities.as_ref())
            .map(|modalities| {
                modalities
                    .iter()
                    .filter_map(|s| parse_modality(s))
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        let output_modalities = self
            .architecture
            .as_ref()
            .and_then(|arch| arch.output_modalities.as_ref())
            .map(|modalities| {
                modalities
                    .iter()
                    .filter_map(|s| parse_modality(s))
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        let supported_features = self
            .supported_parameters
            .as_ref()
            .and_then(|params| parse_features_from_params(params));

        // 将 pricing 转换为 JSON Value
        let pricing = serde_json::to_value(&self.pricing).ok();

        // 构建参数列表
        let parameters = build_parameters(&self.default_parameters, &self.supported_parameters);

        LlmStandardModel {
            id: self.id,
            name: self.name,
            context_length: Some(self.context_length),
            output_token_limit,
            input_cost: Some(self.pricing.prompt),
            output_cost: Some(self.pricing.completion),
            supported_features,
            description: self.description,
            input_modalities,
            output_modalities,
            metadata: None, // 不再需要存储原始数据
            pricing,
            parameters,
        }
    }
}

/// 解析模态字符串
fn parse_modality(s: &str) -> Option<LlmModelModality> {
    match s.to_lowercase().as_str() {
        "text" => Some(LlmModelModality::Text),
        "image" => Some(LlmModelModality::Image),
        "file" => Some(LlmModelModality::File),
        "audio" => Some(LlmModelModality::Audio),
        "video" => Some(LlmModelModality::Video),
        _ => None,
    }
}

/// 从支持的参数列表中解析功能
fn parse_features_from_params(params: &[OpenRouterParameter]) -> Option<Vec<LlmModelFeature>> {
    let mut features = Vec::new();

    for param in params {
        match param {
            // 工具调用相关参数
            OpenRouterParameter::Tools | OpenRouterParameter::ToolChoice => {
                if !features.contains(&LlmModelFeature::Tool) {
                    features.push(LlmModelFeature::Tool);
                }
            }
            // 推理相关参数
            OpenRouterParameter::Reasoning | OpenRouterParameter::IncludeReasoning => {
                if !features.contains(&LlmModelFeature::Reasoning) {
                    features.push(LlmModelFeature::Reasoning);
                }
            }
            // 其他参数暂不处理
            _ => {}
        }
    }

    if features.is_empty() {
        None
    } else {
        Some(features)
    }
}

/// 从 default_parameters 和 supported_parameters 构建参数列表
fn build_parameters(
    default_params: &Option<OpenRouterDefaultParameters>,
    supported_params: &Option<Vec<OpenRouterParameter>>,
) -> Option<Vec<crate::types::ModelParameter>> {
    use crate::types::ModelParameter;

    let mut parameters = Vec::new();

    // 检查哪些参数被支持
    let supported_set = supported_params.as_ref().map(|params| {
        params
            .iter()
            .map(|p| match p {
                OpenRouterParameter::Temperature => "temperature",
                OpenRouterParameter::TopP => "top_p",
                OpenRouterParameter::FrequencyPenalty => "frequency_penalty",
                OpenRouterParameter::PresencePenalty => "presence_penalty",
                OpenRouterParameter::Stop => "stop",
                OpenRouterParameter::MaxTokens => "max_tokens",
                OpenRouterParameter::Seed => "seed",
                _ => "",
            })
            .filter(|s| !s.is_empty())
            .collect::<std::collections::HashSet<_>>()
    });

    // 如果有默认参数，从中提取
    if let Some(defaults) = default_params {
        if let Some(temp) = defaults.temperature {
            if supported_set
                .as_ref()
                .map_or(true, |set| set.contains("temperature"))
            {
                parameters.push(ModelParameter {
                    name: "temperature".to_string(),
                    default: Some(serde_json::json!(temp)),
                    min: Some(serde_json::json!(0.0)),
                    max: Some(serde_json::json!(2.0)),
                });
            }
        }

        if let Some(top_p) = defaults.top_p {
            if supported_set
                .as_ref()
                .map_or(true, |set| set.contains("top_p"))
            {
                parameters.push(ModelParameter {
                    name: "top_p".to_string(),
                    default: Some(serde_json::json!(top_p)),
                    min: Some(serde_json::json!(0.0)),
                    max: Some(serde_json::json!(1.0)),
                });
            }
        }

        if let Some(top_k) = defaults.top_k {
            parameters.push(ModelParameter {
                name: "top_k".to_string(),
                default: Some(serde_json::json!(top_k)),
                min: Some(serde_json::json!(0.0)),
                max: None,
            });
        }

        if let Some(freq_penalty) = defaults.frequency_penalty {
            if supported_set
                .as_ref()
                .map_or(true, |set| set.contains("frequency_penalty"))
            {
                parameters.push(ModelParameter {
                    name: "frequency_penalty".to_string(),
                    default: Some(serde_json::json!(freq_penalty)),
                    min: Some(serde_json::json!(-2.0)),
                    max: Some(serde_json::json!(2.0)),
                });
            }
        }

        if let Some(pres_penalty) = defaults.presence_penalty {
            if supported_set
                .as_ref()
                .map_or(true, |set| set.contains("presence_penalty"))
            {
                parameters.push(ModelParameter {
                    name: "presence_penalty".to_string(),
                    default: Some(serde_json::json!(pres_penalty)),
                    min: Some(serde_json::json!(-2.0)),
                    max: Some(serde_json::json!(2.0)),
                });
            }
        }

        if let Some(rep_penalty) = defaults.repetition_penalty {
            parameters.push(ModelParameter {
                name: "repetition_penalty".to_string(),
                default: Some(serde_json::json!(rep_penalty)),
                min: Some(serde_json::json!(0.0)),
                max: Some(serde_json::json!(2.0)),
            });
        }

        if let Some(min_p) = defaults.min_p {
            parameters.push(ModelParameter {
                name: "min_p".to_string(),
                default: Some(serde_json::json!(min_p)),
                min: Some(serde_json::json!(0.0)),
                max: Some(serde_json::json!(1.0)),
            });
        }

        if let Some(top_a) = defaults.top_a {
            parameters.push(ModelParameter {
                name: "top_a".to_string(),
                default: Some(serde_json::json!(top_a)),
                min: Some(serde_json::json!(0.0)),
                max: Some(serde_json::json!(1.0)),
            });
        }
    }

    if parameters.is_empty() {
        None
    } else {
        Some(parameters)
    }
}

/// OpenRouter 模型客户端
pub struct OpenRouterModelClient {
    client: reqwest::Client,
}

impl OpenRouterModelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for OpenRouterModelClient {
    async fn list_models(
        &self,
        provider: &LlmProvider,
        _provider_type: &str,
    ) -> Result<Vec<LlmStandardModel>, LlmClientError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenRouter models from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                LlmClientError::transport(format!("Failed to fetch OpenRouter models: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "OpenRouter API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: OpenRouterModelsResponse = response.json().await.map_err(|e| {
            LlmClientError::unexpected(format!("Failed to parse OpenRouter response: {}", e))
        })?;

        // 直接转换为标准模型
        let standard_models = models_response
            .data
            .into_iter()
            .map(|model| model.to_standard_model())
            .collect();

        Ok(standard_models)
    }
}
