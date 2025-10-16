/**
 * 用户认证 API
 *
 * 提供用户登录、登出、令牌刷新等功能的前端封装
 */

import { apiCall } from './index';
import type {
  AuthResponse,
  GoogleLoginRequest,
  RefreshTokenRequest,
  UpdateUserProfileRequest,
  User
} from '$lib/types/user';

/**
 * Google OAuth 登录
 *
 * @param request - Google 登录请求参数
 * @returns 认证响应，包含用户信息和令牌
 *
 * 后端接口约定:
 * - 命令: auth_google_login
 * - 参数: { code: string, redirectUri: string }
 * - 返回: AuthResponse
 */
export async function googleLogin(request: GoogleLoginRequest): Promise<AuthResponse> {
  return apiCall<AuthResponse>('auth_google_login', {
    code: request.code,
    redirect_uri: request.redirectUri
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
 * 刷新访问令牌
 *
 * @param request - 刷新令牌请求
 * @returns 新的认证响应
 *
 * 后端接口约定:
 * - 命令: auth_refresh_token
 * - 参数: { refreshToken: string }
 * - 返回: AuthResponse
 */
export async function refreshToken(request: RefreshTokenRequest): Promise<AuthResponse> {
  return apiCall<AuthResponse>('auth_refresh_token', {
    refresh_token: request.refreshToken
  });
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
 * - 参数: { username?: string, avatar?: string }
 * - 返回: User
 */
export async function updateUserProfile(request: UpdateUserProfileRequest): Promise<User> {
  return apiCall<User>('auth_update_profile', {
    username: request.username,
    avatar: request.avatar
  });
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
