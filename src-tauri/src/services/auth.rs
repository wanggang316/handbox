use crate::models::{AppError, AuthResponse, User};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use tiny_http::{Response, Server};
use tokio::task;
use url::Url;

/// Google OAuth 配置
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_port: u16,
}

impl GoogleOAuthConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self, AppError> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| AppError {
            code: "CONFIG_ERROR".to_string(),
            message: "GOOGLE_CLIENT_ID 环境变量未设置".to_string(),
            hint: Some("请在 .env 文件中配置 GOOGLE_CLIENT_ID".to_string()),
        })?;

        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| AppError {
            code: "CONFIG_ERROR".to_string(),
            message: "GOOGLE_CLIENT_SECRET 环境变量未设置".to_string(),
            hint: Some("请在 .env 文件中配置 GOOGLE_CLIENT_SECRET".to_string()),
        })?;

        let redirect_port = std::env::var("OAUTH_REDIRECT_PORT")
            .unwrap_or_else(|_| "14725".to_string())
            .parse()
            .unwrap_or(14725);

        Ok(Self {
            client_id,
            client_secret,
            redirect_port,
        })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}", self.redirect_port)
    }
}

/// Google Token 响应
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    id_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

/// Google ID Token Claims
#[derive(Debug, Deserialize)]
struct GoogleIdTokenClaims {
    sub: String,             // Google User ID
    email: String,           // 用户邮箱
    name: Option<String>,    // 用户名
    picture: Option<String>, // 头像 URL
}

/// Google OAuth 服务
pub struct GoogleOAuthService {
    config: GoogleOAuthConfig,
    http_client: OnceCell<reqwest::Client>,
}

impl GoogleOAuthService {
    /// 创建新的 OAuth 服务实例
    pub fn new() -> Result<Self, AppError> {
        let config = GoogleOAuthConfig::from_env()?;

        Ok(Self {
            config,
            http_client: OnceCell::new(),
        })
    }

    fn http_client(&self) -> Result<&reqwest::Client, AppError> {
        self.http_client.get_or_try_init(|| {
            reqwest::Client::builder().build().map_err(|e| AppError {
                code: "NETWORK_ERROR".to_string(),
                message: format!("创建 HTTP 客户端失败: {}", e),
                hint: None,
            })
        })
    }

    /// 生成授权 URL
    pub fn generate_auth_url(&self) -> Result<String, AppError> {
        let mut auth_url =
            Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|e| AppError {
                code: "URL_PARSE_ERROR".to_string(),
                message: format!("构建授权 URL 失败: {}", e),
                hint: None,
            })?;

        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri())
            .append_pair("response_type", "code")
            .append_pair("scope", "openid email profile")
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent"); // 确保总是返回 refresh_token

        Ok(auth_url.to_string())
    }

    /// 启动本地回调服务器监听授权码
    pub async fn start_callback_server(&self) -> Result<String, AppError> {
        let bind_address = format!("127.0.0.1:{}", self.config.redirect_port);
        let server = Server::http(&bind_address).map_err(|e| AppError {
            code: "SERVER_START_ERROR".to_string(),
            message: format!("启动本地回调服务器失败: {}", e),
            hint: Some(format!("请确保端口 {} 未被占用", self.config.redirect_port)),
        })?;

        tracing::info!("OAuth 回调服务器已启动: http://{}", bind_address);

        // 使用 tokio::task::spawn_blocking 在后台运行阻塞的 HTTP 服务器
        let code = task::spawn_blocking(move || {
            // 设置 120 秒超时
            match server.recv_timeout(Duration::from_secs(120)) {
                Ok(Some(req)) => {
                    tracing::info!("收到回调请求: {}", req.url());

                    // 解析授权码并进行 URL 解码
                    let code = req
                        .url()
                        .split('?')
                        .nth(1)
                        .and_then(|params| {
                            params
                                .split('&')
                                .find(|p| p.starts_with("code="))
                                .and_then(|c| c.split('=').nth(1))
                        })
                        .and_then(|encoded_code| {
                            // URL 解码授权码
                            urlencoding::decode(encoded_code).ok().map(|s| s.to_string())
                        });

                    if let Some(ref c) = code {
                        tracing::info!("成功解析授权码，长度: {}", c.len());
                    } else {
                        tracing::error!("未能解析授权码");
                    }

                    // 返回友好的 HTML 响应
                    let html_response = r#"
                        <!DOCTYPE html>
                        <html>
                        <head>
                            <meta charset="UTF-8">
                            <title>认证成功</title>
                            <style>
                                body {
                                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                                    display: flex;
                                    align-items: center;
                                    justify-content: center;
                                    height: 100vh;
                                    margin: 0;
                                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                                }
                                .container {
                                    background: white;
                                    padding: 2rem;
                                    border-radius: 10px;
                                    box-shadow: 0 10px 40px rgba(0,0,0,0.2);
                                    text-align: center;
                                }
                                h1 { color: #667eea; margin: 0 0 1rem 0; }
                                p { color: #666; margin: 0; }
                            </style>
                        </head>
                        <body>
                            <div class="container">
                                <h1>✓ 认证成功！</h1>
                                <p>您可以关闭此窗口，返回应用继续操作。</p>
                            </div>
                            <script>
                                setTimeout(() => window.close(), 2000);
                            </script>
                        </body>
                        </html>
                    "#;

                    let response = Response::from_string(html_response).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    );

                    let _ = req.respond(response);
                    code
                }
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("接收回调请求失败: {}", e);
                    None
                }
            }
        })
        .await
        .map_err(|e| AppError {
            code: "SERVER_ERROR".to_string(),
            message: format!("回调服务器异常: {}", e),
            hint: None,
        })?;

        code.ok_or_else(|| AppError {
            code: "AUTH_TIMEOUT".to_string(),
            message: "等待授权码超时（120秒）".to_string(),
            hint: Some("请重新尝试登录流程".to_string()),
        })
    }

    /// 使用授权码交换访问令牌
    async fn exchange_code_for_token(&self, code: &str) -> Result<TokenResponse, AppError> {
        let redirect_uri = self.config.redirect_uri();

        tracing::info!("正在交换授权码获取 token...");
        tracing::debug!("授权码: {}...", &code[..code.len().min(20)]);
        tracing::debug!("重定向 URI: {}", redirect_uri);

        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("client_secret", self.config.client_secret.as_str());
        params.insert("code", code);
        params.insert("grant_type", "authorization_code");
        params.insert("redirect_uri", &redirect_uri);

        let client = self.http_client()?;

        let response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError {
                code: "NETWORK_ERROR".to_string(),
                message: format!("请求 Google token 端点失败: {}", e),
                hint: None,
            })?;

        let status = response.status();
        tracing::info!("Google token 端点响应状态: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Google 返回错误: {}", error_text);
            return Err(AppError {
                code: "OAUTH_ERROR".to_string(),
                message: "交换授权码失败".to_string(),
                hint: Some(format!("Google 返回错误: {}", error_text)),
            });
        }

        response
            .json::<TokenResponse>()
            .await
            .map_err(|e| AppError {
                code: "PARSE_ERROR".to_string(),
                message: format!("解析 token 响应失败: {}", e),
                hint: None,
            })
    }

    /// 验证并解析 ID Token
    async fn verify_id_token(&self, id_token: &str) -> Result<GoogleIdTokenClaims, AppError> {
        // 1. 解码 header 获取 kid (key id)
        let header = decode_header(id_token).map_err(|e| AppError {
            code: "TOKEN_INVALID".to_string(),
            message: format!("ID Token header 无效: {}", e),
            hint: None,
        })?;

        let kid = header.kid.ok_or_else(|| AppError {
            code: "TOKEN_INVALID".to_string(),
            message: "ID Token 缺少 kid 字段".to_string(),
            hint: None,
        })?;

        // 2. 获取 Google 的公钥
        let jwks_url = "https://www.googleapis.com/oauth2/v3/certs";
        let client = self.http_client()?;
        let jwks: serde_json::Value = client
            .get(jwks_url)
            .send()
            .await
            .map_err(|e| AppError {
                code: "NETWORK_ERROR".to_string(),
                message: format!("获取 Google JWKS 失败: {}", e),
                hint: None,
            })?
            .json()
            .await
            .map_err(|e| AppError {
                code: "PARSE_ERROR".to_string(),
                message: format!("解析 JWKS 失败: {}", e),
                hint: None,
            })?;

        // 3. 找到匹配的公钥
        let key = jwks["keys"]
            .as_array()
            .and_then(|keys| keys.iter().find(|k| k["kid"].as_str() == Some(&kid)))
            .ok_or_else(|| AppError {
                code: "TOKEN_INVALID".to_string(),
                message: "未找到匹配的公钥".to_string(),
                hint: None,
            })?;

        // 4. 提取 n 和 e 用于 RSA 验证
        let n = key["n"].as_str().ok_or_else(|| AppError {
            code: "TOKEN_INVALID".to_string(),
            message: "公钥缺少 n 参数".to_string(),
            hint: None,
        })?;

        let e = key["e"].as_str().ok_or_else(|| AppError {
            code: "TOKEN_INVALID".to_string(),
            message: "公钥缺少 e 参数".to_string(),
            hint: None,
        })?;

        // 5. 验证 token
        let decoding_key = DecodingKey::from_rsa_components(n, e).map_err(|e| AppError {
            code: "TOKEN_INVALID".to_string(),
            message: format!("创建解码密钥失败: {}", e),
            hint: None,
        })?;

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&[&self.config.client_id]);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);

        let token_data = decode::<GoogleIdTokenClaims>(id_token, &decoding_key, &validation)
            .map_err(|e| AppError {
                code: "TOKEN_INVALID".to_string(),
                message: format!("ID Token 验证失败: {}", e),
                hint: None,
            })?;

        Ok(token_data.claims)
    }

    /// 完整的 Google 登录流程
    pub async fn google_login(&self, code: &str) -> Result<AuthResponse, AppError> {
        // 1. 交换授权码获取 token
        let token_response = self.exchange_code_for_token(code).await?;
        tracing::info!(
            "成功获取 access_token，expires_in: {}",
            token_response.expires_in
        );

        // 2. 验证并解析 ID token
        let claims = self.verify_id_token(&token_response.id_token).await?;
        tracing::info!("ID Token 验证成功");
        tracing::info!(
            "用户信息 - Google ID: {}, Email: {}, Name: {:?}",
            claims.sub,
            claims.email,
            claims.name
        );

        // 3. 构建用户对象
        let timestamp = chrono::Utc::now().to_rfc3339();

        let user = User {
            id: claims.sub.clone(), // 使用 Google User ID
            username: claims.name.clone().unwrap_or_else(|| claims.email.clone()),
            email: claims.email.clone(),
            avatar: claims.picture.clone(),
            is_pro: false, // 默认非 Pro 用户
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };

        tracing::info!(
            "构建用户对象成功 - ID: {}, Username: {}, Email: {}",
            user.id,
            user.username,
            user.email
        );

        // 4. 构建认证响应
        let auth_response = AuthResponse {
            user,
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token.unwrap_or_default(),
            expires_in: token_response.expires_in,
        };

        tracing::info!("Google 登录流程完成");
        Ok(auth_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    static ENV_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn test_oauth_config_redirect_uri() {
        let config = GoogleOAuthConfig {
            client_id: "test_client_id".to_string(),
            client_secret: "test_secret".to_string(),
            redirect_port: 14725,
        };

        assert_eq!(config.redirect_uri(), "http://127.0.0.1:14725");
    }

    #[tokio::test]
    async fn test_generate_auth_url() {
        let _lock = ENV_GUARD.lock().unwrap();
        // 设置测试环境变量
        std::env::set_var("GOOGLE_CLIENT_ID", "test_client_id");
        std::env::set_var("GOOGLE_CLIENT_SECRET", "test_secret");
        std::env::remove_var("OAUTH_REDIRECT_PORT");

        let service = GoogleOAuthService::new().unwrap();
        let auth_url = service.generate_auth_url().unwrap();

        let parsed = Url::parse(&auth_url).expect("valid auth url");
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.domain(), Some("accounts.google.com"));
        assert_eq!(parsed.path(), "/o/oauth2/v2/auth");

        let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

        assert_eq!(params.get("client_id"), Some(&"test_client_id".to_string()));
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));
        assert_eq!(
            params.get("scope"),
            Some(&"openid email profile".to_string())
        );

        // 清理环境
        std::env::remove_var("GOOGLE_CLIENT_ID");
        std::env::remove_var("GOOGLE_CLIENT_SECRET");
    }

    #[test]
    fn test_oauth_config_custom_port() {
        let _lock = ENV_GUARD.lock().unwrap();
        std::env::set_var("OAUTH_REDIRECT_PORT", "8080");
        std::env::set_var("GOOGLE_CLIENT_ID", "test_id");
        std::env::set_var("GOOGLE_CLIENT_SECRET", "test_secret");

        let config = GoogleOAuthConfig::from_env().unwrap();
        assert_eq!(config.redirect_port, 8080);
        assert_eq!(config.redirect_uri(), "http://127.0.0.1:8080");

        std::env::remove_var("OAUTH_REDIRECT_PORT");
        std::env::remove_var("GOOGLE_CLIENT_ID");
        std::env::remove_var("GOOGLE_CLIENT_SECRET");
    }
}
