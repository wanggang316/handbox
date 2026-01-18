/**
 * 图片相关命令
 *
 * 提供图片代理加载功能，解决第三方图片服务（如 Google）的访问限制问题
 */
use crate::models::error::AppError;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};

/// 代理加载图片
///
/// 通过后端代理方式加载外部图片，避免 WebView 的访问限制
///
/// # 参数
/// - `url`: 图片 URL
///
/// # 返回
/// - `Vec<u8>`: 图片二进制数据
///
/// # 错误
/// - `VALIDATION_ERROR`: URL 格式无效
/// - `NETWORK_ERROR`: 网络请求失败
/// - `RATE_LIMIT`: 被限流（429）
#[tauri::command]
pub async fn image_proxy(url: String) -> Result<Vec<u8>, AppError> {
    // 验证 URL 格式
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|e| AppError::validation_error(&format!("无效的图片 URL: {e}")))?;

    // 只允许 HTTPS 协议
    if parsed_url.scheme() != "https" {
        return Err(AppError::with_hint(
            "VALIDATION_ERROR",
            "只允许加载 HTTPS 图片",
            "出于安全考虑，仅支持 HTTPS 协议的图片",
        ));
    }

    // 构建 HTTP 客户端
    let client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));
    #[cfg(test)]
    let client_builder = client_builder.no_proxy();
    let client = client_builder
        .build()
        .map_err(|e| AppError::internal_error(&format!("创建 HTTP 客户端失败: {e}")))?;

    // 构建请求头
    let mut headers = HeaderMap::new();

    // 使用标准浏览器 User-Agent
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        ),
    );

    // 添加 Referer 头（模拟浏览器访问）
    if parsed_url
        .host_str()
        .unwrap_or("")
        .contains("googleusercontent.com")
    {
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://accounts.google.com/"),
        );
    }

    // 发起请求
    let response = client
        .get(url.clone())
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::network_error(&format!("图片请求失败: {e}")))?;

    // 检查响应状态
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            429 => AppError::rate_limit_error(),
            401 | 403 => AppError::with_hint(
                "AUTH_ERROR",
                &format!("无权访问此图片，HTTP 状态码: {status}"),
                "请检查图片链接的访问权限",
            ),
            _ => AppError::network_error(&format!("图片加载失败，HTTP 状态码: {status}")),
        });
    }

    // 获取图片数据
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::network_error(&format!("读取图片数据失败: {e}")))?;

    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_image_proxy_invalid_url() {
        let result = image_proxy("not-a-url".to_string()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn test_image_proxy_http_not_allowed() {
        let result = image_proxy("http://example.com/image.jpg".to_string()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("HTTPS"));
    }

    #[tokio::test]
    async fn test_image_proxy_invalid_domain() {
        let result = image_proxy("https://invalid-domain-12345.com/image.jpg".to_string()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "NETWORK_ERROR");
    }
}
