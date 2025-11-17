/**
 * 用户认证状态管理
 *
 * 统一管理用户登录、登出、会话恢复等逻辑
 */

import { startGoogleOAuth, onLoginSuccess, onLoginError, getCurrentUser, logout as apiLogout } from '$lib/api/auth';
import type { User, AuthResponse } from '$lib/types/user';

interface AuthState {
  user: User | null;
  isLoggedIn: boolean;
  isLoading: boolean;
  error: string | null;
}

// 初始化状态
const initialState: AuthState = $state({
  user: null,
  isLoggedIn: false,
  isLoading: false,
  error: null
});

// 创建响应式状态
export const authState = initialState;

// 初始化标志
let initialized = false;
let loginSuccessUnlisten: (() => void) | undefined;
let loginErrorUnlisten: (() => void) | undefined;

/**
 * 初始化认证状态
 * - 恢复上次的用户会话
 * - 设置事件监听器
 */
export async function initAuth() {
  if (initialized) return;

  console.log('[Auth] 初始化认证状态...');

  try {
    // 尝试恢复会话
    const user = await getCurrentUser();
    authState.user = user;
    authState.isLoggedIn = true;
    console.log('[Auth] 会话恢复成功:', user.email);
  } catch (error) {
    console.log('[Auth] 无活跃会话');
    authState.user = null;
    authState.isLoggedIn = false;
  }

  // 设置登录成功事件监听
  loginSuccessUnlisten = await onLoginSuccess((authResponse: AuthResponse) => {
    console.log('[Auth] 登录成功:', authResponse.user.email);
    authState.user = authResponse.user;
    authState.isLoggedIn = true;
    authState.isLoading = false;
    authState.error = null;

    // 保存刷新令牌到 localStorage
    if (typeof window !== 'undefined' && authResponse.refreshToken) {
      localStorage.setItem('refreshToken', authResponse.refreshToken);
    }
  });

  // 设置登录失败事件监听
  loginErrorUnlisten = await onLoginError((error) => {
    console.error('[Auth] 登录失败:', error);
    authState.isLoading = false;
    authState.error = error.message;
  });

  initialized = true;
  console.log('[Auth] 初始化完成');
}

/**
 * 清理认证状态（应用卸载时调用）
 */
export function cleanupAuth() {
  loginSuccessUnlisten?.();
  loginErrorUnlisten?.();
  initialized = false;
  console.log('[Auth] 清理完成');
}

/**
 * 启动 Google OAuth 登录
 */
export async function login() {
  console.log('[Auth] 启动 Google OAuth 登录...');
  authState.isLoading = true;
  authState.error = null;

  try {
    await startGoogleOAuth();
    // 登录结果会通过事件回调处理
  } catch (error) {
    console.error('[Auth] 启动登录失败:', error);
    authState.isLoading = false;
    authState.error = '启动登录失败，请重试';
  }
}

/**
 * 退出登录
 */
export async function logout() {
  console.log('[Auth] 退出登录...');
  authState.isLoading = true;
  authState.error = null;

  try {
    await apiLogout();
    authState.user = null;
    authState.isLoggedIn = false;
    authState.isLoading = false;

    // 清除本地存储
    if (typeof window !== 'undefined') {
      localStorage.removeItem('refreshToken');
    }

    console.log('[Auth] 退出成功');
  } catch (error) {
    console.error('[Auth] 退出失败:', error);
    authState.isLoading = false;
    authState.error = '退出失败，请重试';
  }
}

/**
 * 更新用户信息
 */
export function updateUser(user: User) {
  authState.user = user;
  console.log('[Auth] 用户信息已更新');
}

/**
 * 清除错误信息
 */
export function clearError() {
  authState.error = null;
}
