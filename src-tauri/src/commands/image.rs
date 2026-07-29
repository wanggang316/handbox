use crate::models::error::AppError;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};

/// Loads an external image through the backend, bypassing WebView access
/// restrictions of third-party image hosts (e.g. Google).
///
/// # Errors
/// - `VALIDATION_ERROR`: invalid or non-HTTPS URL
/// - `NETWORK_ERROR`: request failure or non-OK status
/// - `RATE_LIMIT`: throttled (429); `AUTH_ERROR`: 401/403
#[tauri::command]
pub async fn image_proxy(url: String) -> Result<Vec<u8>, AppError> {
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|e| AppError::validation_error(&format!("无效的图片 URL: {e}")))?;

    if parsed_url.scheme() != "https" {
        return Err(AppError::with_hint(
            "VALIDATION_ERROR",
            "只允许加载 HTTPS 图片",
            "出于安全考虑，仅支持 HTTPS 协议的图片",
        ));
    }

    let client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));
    #[cfg(test)]
    let client_builder = client_builder.no_proxy();
    let client = client_builder
        .build()
        .map_err(|e| AppError::internal_error(&format!("创建 HTTP 客户端失败: {e}")))?;

    let mut headers = HeaderMap::new();

    // Browser-like User-Agent: some image hosts reject non-browser clients.
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        ),
    );

    // Google-hosted images expect a plausible Referer.
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

    let response = client
        .get(url.clone())
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::network_error(&format!("图片请求失败: {e}")))?;

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
