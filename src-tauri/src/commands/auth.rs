use crate::models::{AppError, AuthResponse, GoogleLoginRequest, UpdateUserProfileRequest, User};
use crate::services::{GoogleOAuthService, UserSessionService};
use tauri::{Emitter, State};

/// Generates the auth URL, opens the browser, and starts a local callback
/// server in the background that completes the login and emits
/// `auth_login_success` / `auth_login_error` to the frontend.
#[tauri::command]
pub async fn auth_start_google_oauth(
    window: tauri::Window,
    session_service: State<'_, UserSessionService>,
) -> Result<String, AppError> {
    tracing::info!("启动 Google OAuth 登录流程");

    let oauth_service = GoogleOAuthService::new()?;
    let auth_url = oauth_service.generate_auth_url()?;

    let oauth_service_clone = GoogleOAuthService::new()?;
    let window_clone = window.clone();
    let session_service_clone = session_service.inner().clone();

    tokio::spawn(async move {
        match oauth_service_clone.start_callback_server().await {
            Ok(code) => {
                tracing::info!("成功接收到授权码");
                match oauth_service_clone.google_login(&code).await {
                    Ok(auth_response) => {
                        tracing::info!(
                            "Google 登录成功: user_id={}, email={}",
                            auth_response.user.id,
                            auth_response.user.email
                        );

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

                        let _ = window_clone.emit("auth_login_success", auth_response);
                    }
                    Err(e) => {
                        tracing::error!("Google 登录失败: {:?}", e);
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

/// Logs in with an authorization code directly, for custom OAuth flows; the
/// normal path is `auth_start_google_oauth`.
#[tauri::command]
pub async fn auth_google_login(
    request: GoogleLoginRequest,
    session_service: State<'_, UserSessionService>,
) -> Result<AuthResponse, AppError> {
    tracing::info!("开始 Google OAuth 登录流程（直接授权码方式）");

    let oauth_service = GoogleOAuthService::new()?;
    let auth_response = oauth_service.google_login(&request.code).await?;

    tracing::info!(
        "Google 登录成功: user_id={}, email={}",
        auth_response.user.id,
        auth_response.user.email
    );

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

#[tauri::command]
pub async fn auth_logout(session_service: State<'_, UserSessionService>) -> Result<(), AppError> {
    session_service.clear_session().await?;
    tracing::info!("用户已登出");
    Ok(())
}

#[tauri::command]
pub async fn auth_refresh_token(
    session_service: State<'_, UserSessionService>,
) -> Result<(), AppError> {
    session_service.refresh_google_token().await?;
    tracing::info!("Google Access Token 已刷新");
    Ok(())
}

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

#[tauri::command]
pub async fn auth_update_profile(
    request: UpdateUserProfileRequest,
    session_service: State<'_, UserSessionService>,
) -> Result<User, AppError> {
    let mut current_user = session_service
        .get_current_user()
        .await
        .ok_or_else(|| AppError {
            code: "NO_SESSION".to_string(),
            message: "当前没有登录用户".to_string(),
            hint: Some("请先登录".to_string()),
        })?;

    if let Some(username) = request.username {
        current_user.username = username;
    }
    if let Some(avatar) = request.avatar {
        current_user.avatar = Some(avatar);
    }
    current_user.updated_at = chrono::Utc::now().to_rfc3339();

    session_service.update_user(&current_user).await?;

    tracing::info!("用户资料已更新: user_id={}", current_user.id);

    Ok(current_user)
}

#[tauri::command]
pub async fn auth_validate_token(
    session_service: State<'_, UserSessionService>,
) -> Result<bool, AppError> {
    let is_valid = session_service.is_session_valid().await;
    Ok(is_valid)
}
