/**
 * 用户认证 API
 *
 * 提供用户登录、登出、令牌刷新等功能的前端封装
 */

import { apiCall } from './index';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AuthResponse,
  UpdateUserProfileRequest,
  User
} from '$lib/types/user';

/**
 * 启动 Google OAuth 登录流程
 *
 * 此函数会：
 * 1. 生成授权 URL
 * 2. 打开系统浏览器进行授权
 * 3. 启动本地回调服务器等待授权码
 * 4. 通过事件通知登录结果
 *
 * @returns 授权 URL
 *
 * 后端接口约定:
 * - 命令: auth_start_google_oauth
 * - 事件: auth_login_success (成功) / auth_login_error (失败)
 */
export async function startGoogleOAuth(): Promise<string> {
  return apiCall<string>('auth_start_google_oauth');
}

/**
 * 监听 Google OAuth 登录成功事件
 *
 * @param callback - 登录成功回调函数
 * @returns 取消监听函数
 */
export async function onLoginSuccess(
  callback: (authResponse: AuthResponse) => void
): Promise<UnlistenFn> {
  return listen<AuthResponse>('auth_login_success', (event) => {
    callback(event.payload);
  });
}

/**
 * 监听 Google OAuth 登录失败事件
 *
 * @param callback - 登录失败回调函数
 * @returns 取消监听函数
 */
export async function onLoginError(
  callback: (error: { code: string; message: string; hint?: string }) => void
): Promise<UnlistenFn> {
  return listen<{ code: string; message: string; hint?: string }>('auth_login_error', (event) => {
    callback(event.payload);
  });
}

/**
 * 用户登出
 *
 * 清除服务端 session 和本地令牌
 *
 * 后端接口约定:
 * - 命令: auth_logout
 * - 参数: 无
 * - 返回: void
 */
export async function logout(): Promise<void> {
  return apiCall<void>('auth_logout');
}

/**
 * 刷新 Google Access Token
 *
 * 使用当前会话中的 refresh_token 获取新的 access_token
 *
 * 后端接口约定:
 * - 命令: auth_refresh_token
 * - 参数: 无（从会话中自动获取）
 * - 返回: void
 */
export async function refreshToken(): Promise<void> {
  return apiCall<void>('auth_refresh_token');
}

/**
 * 获取当前用户信息
 *
 * 验证当前令牌并返回用户信息
 *
 * 后端接口约定:
 * - 命令: auth_get_user
 * - 参数: 无
 * - 返回: User
 */
export async function getCurrentUser(): Promise<User> {
  return apiCall<User>('auth_get_user');
}

/**
 * 更新用户资料
 *
 * @param request - 更新用户资料请求
 * @returns 更新后的用户信息
 *
 * 后端接口约定:
 * - 命令: auth_update_profile
 * - 参数: { request: { username?: string, avatar?: string } }
 * - 返回: User
 */
export async function updateUserProfile(request: UpdateUserProfileRequest): Promise<User> {
  return apiCall<User>('auth_update_profile', { request });
}

/**
 * 验证令牌有效性
 *
 * @returns 令牌是否有效
 *
 * 后端接口约定:
 * - 命令: auth_validate_token
 * - 参数: 无
 * - 返回: boolean
 */
export async function validateToken(): Promise<boolean> {
  try {
    await apiCall<void>('auth_validate_token');
    return true;
  } catch {
    return false;
  }
}
