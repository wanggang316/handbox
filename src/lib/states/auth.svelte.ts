/**
 * Auth state: login, logout, and session restore.
 */

import { startGoogleOAuth, onLoginSuccess, onLoginError, getCurrentUser, logout as apiLogout } from '$lib/api/auth';
import type { User, AuthResponse } from '$lib/types/user';

interface AuthState {
  user: User | null;
  isLoggedIn: boolean;
  isLoading: boolean;
  error: string | null;
}

const initialState: AuthState = $state({
  user: null,
  isLoggedIn: false,
  isLoading: false,
  error: null
});

export const authState = initialState;

let initialized = false;
let loginSuccessUnlisten: (() => void) | undefined;
let loginErrorUnlisten: (() => void) | undefined;
let authSyncChannel: BroadcastChannel | null = null;
let authSyncStorageHandler: ((event: StorageEvent) => void) | null = null;

const AUTH_SYNC_KEY = 'auth_sync_event';
const AUTH_SYNC_CHANNEL = 'handbox_auth_sync';

type AuthSyncEvent = 'login' | 'logout';

function emitAuthSync(event: AuthSyncEvent) {
  if (typeof window === 'undefined') return;

  const payload = JSON.stringify({ event, ts: Date.now() });
  localStorage.setItem(AUTH_SYNC_KEY, payload);
  authSyncChannel?.postMessage(payload);
}

async function handleAuthSync(rawPayload: unknown) {
  if (!rawPayload || typeof window === 'undefined') return;

  let payload: { event?: AuthSyncEvent } | null = null;
  try {
    if (typeof rawPayload === 'string') {
      payload = JSON.parse(rawPayload);
    } else if (typeof rawPayload === 'object') {
      payload = rawPayload as { event?: AuthSyncEvent };
    }
  } catch (error) {
    console.warn('[Auth] 同步事件解析失败:', error);
    return;
  }

  if (!payload?.event) return;

  if (payload.event === 'logout') {
    authState.user = null;
    authState.isLoggedIn = false;
    authState.isLoading = false;
    authState.error = null;

    if (typeof window !== 'undefined') {
      localStorage.removeItem('refreshToken');
    }
    return;
  }

  if (payload.event === 'login') {
    authState.isLoading = true;
    authState.error = null;

    try {
      const user = await getCurrentUser();
      authState.user = user;
      authState.isLoggedIn = true;
    } catch (error) {
      console.warn('[Auth] 同步登录状态失败:', error);
      authState.user = null;
      authState.isLoggedIn = false;
    } finally {
      authState.isLoading = false;
    }
  }
}

/**
 * Initialize auth: restore the previous user session and set up event
 * listeners.
 */
export async function initAuth() {
  if (initialized) return;

  console.log('[Auth] 初始化认证状态...');

  try {
    const user = await getCurrentUser();
    authState.user = user;
    authState.isLoggedIn = true;
    console.log('[Auth] 会话恢复成功:', user.email);
  } catch (error) {
    console.log('[Auth] 无活跃会话');
    authState.user = null;
    authState.isLoggedIn = false;
  }

  loginSuccessUnlisten = await onLoginSuccess((authResponse: AuthResponse) => {
    console.log('[Auth] 登录成功:', authResponse.user.email);
    authState.user = authResponse.user;
    authState.isLoggedIn = true;
    authState.isLoading = false;
    authState.error = null;

    if (typeof window !== 'undefined' && authResponse.refreshToken) {
      localStorage.setItem('refreshToken', authResponse.refreshToken);
    }

    emitAuthSync('login');
  });

  loginErrorUnlisten = await onLoginError((error) => {
    console.error('[Auth] 登录失败:', error);
    authState.isLoading = false;
    authState.error = error.message;
  });

  if (typeof window !== 'undefined') {
    if ('BroadcastChannel' in window) {
      authSyncChannel = new BroadcastChannel(AUTH_SYNC_CHANNEL);
      authSyncChannel.onmessage = (event) => {
        handleAuthSync(event.data);
      };
    }

    authSyncStorageHandler = (event) => {
      if (event.key === AUTH_SYNC_KEY && event.newValue) {
        handleAuthSync(event.newValue);
      }
    };
    window.addEventListener('storage', authSyncStorageHandler);
  }

  initialized = true;
  console.log('[Auth] 初始化完成');
}

/** Tear down auth listeners and sync channels (called on app unload). */
export function cleanupAuth() {
  loginSuccessUnlisten?.();
  loginErrorUnlisten?.();
  if (typeof window !== 'undefined' && authSyncStorageHandler) {
    window.removeEventListener('storage', authSyncStorageHandler);
  }
  authSyncStorageHandler = null;
  authSyncChannel?.close();
  authSyncChannel = null;
  initialized = false;
  console.log('[Auth] 清理完成');
}

/** Start Google OAuth login. */
export async function login() {
  console.log('[Auth] 启动 Google OAuth 登录...');
  authState.isLoading = true;
  authState.error = null;

  try {
    await startGoogleOAuth();
    // The login result arrives via the event callbacks.
  } catch (error) {
    console.error('[Auth] 启动登录失败:', error);
    authState.isLoading = false;
    authState.error = '启动登录失败，请重试';
  }
}

export async function logout() {
  console.log('[Auth] 退出登录...');
  authState.isLoading = true;
  authState.error = null;

  try {
    await apiLogout();
    authState.user = null;
    authState.isLoggedIn = false;
    authState.isLoading = false;

    if (typeof window !== 'undefined') {
      localStorage.removeItem('refreshToken');
    }

    emitAuthSync('logout');
    console.log('[Auth] 退出成功');
  } catch (error) {
    console.error('[Auth] 退出失败:', error);
    authState.isLoading = false;
    authState.error = '退出失败，请重试';
  }
}

export async function confirmLogout(message = '确定要退出登录吗？') {
  if (typeof window === 'undefined') return true;

  try {
    const { confirm } = await import('@tauri-apps/plugin-dialog');
    return await confirm(message);
  } catch (error) {
    console.warn('[Auth] 使用系统弹窗确认失败，回退到浏览器确认:', error);
    return window.confirm(message);
  }
}

export function updateUser(user: User) {
  authState.user = user;
  console.log('[Auth] 用户信息已更新');
}

export function clearError() {
  authState.error = null;
}
