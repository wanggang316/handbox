
import { apiCall } from './index';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AuthResponse,
  UpdateUserProfileRequest,
  User
} from '$lib/types/user';

/**
 * Opens the system browser for authorization and starts a local callback
 * server; the outcome arrives via the auth_login_success / auth_login_error
 * events. Returns the authorization URL.
 */
export async function startGoogleOAuth(): Promise<string> {
  return apiCall<string>('auth_start_google_oauth');
}

export async function onLoginSuccess(
  callback: (authResponse: AuthResponse) => void
): Promise<UnlistenFn> {
  return listen<AuthResponse>('auth_login_success', (event) => {
    callback(event.payload);
  });
}

export async function onLoginError(
  callback: (error: { code: string; message: string; hint?: string }) => void
): Promise<UnlistenFn> {
  return listen<{ code: string; message: string; hint?: string }>('auth_login_error', (event) => {
    callback(event.payload);
  });
}

/** Clears the server-side session and local tokens. */
export async function logout(): Promise<void> {
  return apiCall<void>('auth_logout');
}

/** Exchanges the session's refresh_token for a new access_token, stored backend-side. */
export async function refreshToken(): Promise<void> {
  return apiCall<void>('auth_refresh_token');
}

export async function getCurrentUser(): Promise<User> {
  return apiCall<User>('auth_get_user');
}

export async function updateUserProfile(request: UpdateUserProfileRequest): Promise<User> {
  return apiCall<User>('auth_update_profile', { request });
}

export async function validateToken(): Promise<boolean> {
  try {
    await apiCall<void>('auth_validate_token');
    return true;
  } catch {
    return false;
  }
}
