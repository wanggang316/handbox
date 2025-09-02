// 模型拉取功能演示
// 
// 运行方式：
// cd src-tauri
// cargo run --example model_fetch_demo

use handbox_lib::models::{ProviderConfig, ModelFeature};
use handbox_lib::services::{DatabaseService, ProviderService};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("🚀 模型拉取功能演示");
    println!("================================");
    
    // 创建临时数据库（每次运行都使用新的数据库）
    let db_path = env::temp_dir().join(format!("demo_{}.db", 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()));
    println!("📄 使用数据库: {:?}", db_path);
    
    let db_service = DatabaseService::new(&db_path).await?;
    let provider_service = ProviderService::new(db_service);
    
    // 1. 演示 Anthropic（使用本地数据库）
    println!("\n📡 创建 Anthropic 供应商（使用本地模型数据库）...");
    let anthropic_config = ProviderConfig {
        name: "Demo Anthropic".to_string(),
        provider_type: "anthropic".to_string(),
        base_url: "https://api.anthropic.com".to_string(),
        api_key: "demo-key".to_string(),
        enabled: Some(true),
    };
    
    let anthropic_provider = provider_service.create_provider(anthropic_config).await?;
    println!("✅ 创建成功: {}", anthropic_provider.name);
    
    // 获取 Anthropic 模型
    let anthropic_models = provider_service.get_provider_models(&anthropic_provider.id, false).await?;
    println!("📋 获取到 {} 个 Anthropic 模型:", anthropic_models.len());
    for model in &anthropic_models {
        println!("  - {} ({})", model.name, model.id);
        if let Some(context) = model.context_length {
            println!("    上下文长度: {} tokens", context);
        }
        if let (Some(input), Some(output)) = (model.input_cost, model.output_cost) {
            println!("    定价: ${:.4}/1K输入, ${:.4}/1K输出", input, output);
        }
        if let Some(features) = &model.supported_features {
            println!("    特性: {:?}", features);
        }
        println!();
    }
    
    // 2. 演示 Google API 调用（预期会因无效密钥失败）
    println!("\n📡 演示 Google AI API 调用（预期会因无效密钥失败）...");
    let google_config = ProviderConfig {
        name: "Demo Google AI".to_string(),
        provider_type: "google".to_string(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        api_key: "invalid-demo-key".to_string(),
        enabled: Some(true),
    };
    
    let google_provider = provider_service.create_provider(google_config).await?;
    println!("✅ Google AI 供应商创建成功: {}", google_provider.name);
    
    match provider_service.get_provider_models(&google_provider.id, true).await {
        Ok(models) => {
            println!("📋 获取到 {} 个 Google AI 模型 (意外成功!)", models.len());
            for model in models.iter().take(3) {
                println!("  - {} ({})", model.name, model.id);
                if let Some(features) = &model.supported_features {
                    if features.contains(&ModelFeature::Reasoning) {
                        println!("    🧠 支持推理模式");
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ 预期的 API 错误: {}", e);
            println!("   这证明了错误处理机制正常工作");
        }
    }
    
    // 3. 演示 OpenAI API 调用（也会因无效密钥失败）
    println!("\n📡 演示 OpenAI API 调用（预期会因无效密钥失败）...");
    let openai_config = ProviderConfig {
        name: "Demo OpenAI Provider".to_string(),
        provider_type: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        api_key: "invalid-demo-key".to_string(),
        enabled: Some(true),
    };
    
    let openai_provider = provider_service.create_provider(openai_config).await?;
    println!("✅ OpenAI 供应商创建成功: {}", openai_provider.name);
    
    match provider_service.get_provider_models(&openai_provider.id, true).await {
        Ok(models) => {
            println!("📋 获取到 {} 个 OpenAI 模型 (意外成功!)", models.len());
        }
        Err(e) => {
            println!("❌ 预期的 API 错误: {}", e);
            println!("   这证明了错误处理机制正常工作");
        }
    }
    
    // 4. 演示供应商更新时自动刷新模型
    println!("\n🔄 演示供应商更新自动刷新模型...");
    let updated_config = ProviderConfig {
        name: "Updated Anthropic".to_string(),
        provider_type: "anthropic".to_string(),
        base_url: "https://api.anthropic.com".to_string(),
        api_key: "new-demo-key".to_string(), // 改变API密钥
        enabled: Some(true),
    };
    
    provider_service.update_provider(&anthropic_provider.id, updated_config).await?;
    println!("✅ 供应商更新完成，模型列表已自动刷新");
    
    println!("\n🎉 演示完成！");
    println!("================================");
    println!("✨ 功能摘要:");
    println!("  - ✅ 自动拉取各供应商模型列表");
    println!("  - ✅ 丰富的模型信息（定价、特性、上下文长度）");
    println!("  - ✅ 创建/更新供应商时自动刷新模型");
    println!("  - ✅ 优雅的错误处理");
    println!("  - ✅ 支持多个供应商: OpenAI, Anthropic, Google, DeepSeek, OpenRouter");
    
    Ok(())
}
