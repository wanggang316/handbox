use crate::models::{AppError, AuthResponse, GoogleLoginRequest, UpdateUserProfileRequest, User};
use crate::services::{GoogleOAuthService, UserSessionService};
use tauri::{Emitter, State};

/// 启动 Google OAuth 登录流程
///
/// 生成授权 URL，打开浏览器，启动本地回调服务器，等待授权码
#[tauri::command]
pub async fn auth_start_google_oauth(
    window: tauri::Window,
    session_service: State<'_, UserSessionService>,
) -> Result<String, AppError> {
    tracing::info!("启动 Google OAuth 登录流程");

    // 1. 创建 OAuth 服务实例
    let oauth_service = GoogleOAuthService::new()?;

    // 2. 生成授权 URL
    let auth_url = oauth_service.generate_auth_url()?;

    // 3. 在后台启动回调服务器
    let oauth_service_clone = GoogleOAuthService::new()?;
    let window_clone = window.clone();
    let session_service_clone = session_service.inner().clone();

    tokio::spawn(async move {
        match oauth_service_clone.start_callback_server().await {
            Ok(code) => {
                tracing::info!("成功接收到授权码");
                // 使用授权码完成登录
                match oauth_service_clone.google_login(&code).await {
                    Ok(auth_response) => {
                        tracing::info!(
                            "Google 登录成功: user_id={}, email={}",
                            auth_response.user.id,
                            auth_response.user.email
                        );

                        // 保存用户会话
                        if let Err(e) = session_service_clone
                            .create_session(
                                auth_response.user.clone(),
                                auth_response.access_token.clone(),
                                auth_response.refresh_token.clone(),
                                auth_response.expires_in,
                            )
                            .await
                        {
                            tracing::error!("保存用户会话失败: {:?}", e);
                        }

                        // 发送成功事件到前端
                        let _ = window_clone.emit("auth_login_success", auth_response);
                    }
                    Err(e) => {
                        tracing::error!("Google 登录失败: {:?}", e);
                        // 发送失败事件到前端
                        let _ = window_clone.emit("auth_login_error", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("接收授权码失败: {:?}", e);
                let _ = window_clone.emit("auth_login_error", e);
            }
        }
    });

    // 4. 使用 tauri-plugin-opener 打开浏览器
    use tauri_plugin_opener::OpenerExt;
    window
        .opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| AppError {
            code: "BROWSER_ERROR".to_string(),
            message: format!("打开浏览器失败: {}", e),
            hint: Some("请手动在浏览器中打开授权链接".to_string()),
        })?;

    Ok(auth_url)
}

/// Google OAuth 登录（备用方法，通常使用 auth_start_google_oauth）
///
/// 直接使用授权码进行登录，适用于自定义 OAuth 流程
#[tauri::command]
pub async fn auth_google_login(
    request: GoogleLoginRequest,
    session_service: State<'_, UserSessionService>,
) -> Result<AuthResponse, AppError> {
    tracing::info!("开始 Google OAuth 登录流程（直接授权码方式）");

    // 1. 创建 OAuth 服务实例
    let oauth_service = GoogleOAuthService::new()?;

    // 2. 使用授权码交换令牌并验证
    let auth_response = oauth_service.google_login(&request.code).await?;

    tracing::info!(
        "Google 登录成功: user_id={}, email={}",
        auth_response.user.id,
        auth_response.user.email
    );

    // 3. 保存用户会话
    session_service
        .create_session(
            auth_response.user.clone(),
            auth_response.access_token.clone(),
            auth_response.refresh_token.clone(),
            auth_response.expires_in,
        )
        .await?;

    Ok(auth_response)
}

/// 用户登出
///
/// 清除用户会话
#[tauri::command]
pub async fn auth_logout(session_service: State<'_, UserSessionService>) -> Result<(), AppError> {
    session_service.clear_session().await?;
    tracing::info!("用户已登出");
    Ok(())
}

/// 刷新 Google Access Token
///
/// 使用 refresh_token 获取新的 access_token
#[tauri::command]
pub async fn auth_refresh_token(
    session_service: State<'_, UserSessionService>,
) -> Result<(), AppError> {
    session_service.refresh_google_token().await?;
    tracing::info!("Google Access Token 已刷新");
    Ok(())
}

/// 获取当前用户信息
///
/// 从会话中获取当前登录的用户信息
#[tauri::command]
pub async fn auth_get_user(
    session_service: State<'_, UserSessionService>,
) -> Result<User, AppError> {
    session_service
        .get_current_user()
        .await
        .ok_or_else(|| AppError {
            code: "NO_SESSION".to_string(),
            message: "当前没有登录用户".to_string(),
            hint: Some("请先登录".to_string()),
        })
}

/// 更新用户资料
///
/// 更新用户名、头像等信息
#[tauri::command]
pub async fn auth_update_profile(
    request: UpdateUserProfileRequest,
    session_service: State<'_, UserSessionService>,
) -> Result<User, AppError> {
    // 1. 获取当前用户
    let mut current_user = session_service
        .get_current_user()
        .await
        .ok_or_else(|| AppError {
            code: "NO_SESSION".to_string(),
            message: "当前没有登录用户".to_string(),
            hint: Some("请先登录".to_string()),
        })?;

    // 2. 更新用户信息
    if let Some(username) = request.username {
        current_user.username = username;
    }
    if let Some(avatar) = request.avatar {
        current_user.avatar = Some(avatar);
    }
    current_user.updated_at = chrono::Utc::now().to_rfc3339();

    // 3. 保存到数据库
    session_service.update_user(&current_user).await?;

    tracing::info!("用户资料已更新: user_id={}", current_user.id);

    Ok(current_user)
}

/// 检查会话是否有效
///
/// 检查当前会话和 token 是否有效（未过期）
#[tauri::command]
pub async fn auth_validate_token(
    session_service: State<'_, UserSessionService>,
) -> Result<bool, AppError> {
    let is_valid = session_service.is_session_valid().await;
    Ok(is_valid)
}
