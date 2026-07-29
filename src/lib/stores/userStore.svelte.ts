/**
 * Global user store - Svelte 5 runes.
 */

import type { User, UserState } from '$lib/types/user';

function createUserStore() {
  let state = $state<UserState>({
    user: null,
    isLoggedIn: false,
    accessToken: null,
    isLoading: false
  });

  // Restore user state from localStorage (browser environment only).
  if (typeof window !== 'undefined') {
    const savedUser = localStorage.getItem('user');
    const savedToken = localStorage.getItem('accessToken');

    if (savedUser && savedToken) {
      try {
        state.user = JSON.parse(savedUser);
        state.accessToken = savedToken;
        state.isLoggedIn = true;
      } catch (error) {
        console.error('恢复用户状态失败:', error);
        // Clear invalid data.
        localStorage.removeItem('user');
        localStorage.removeItem('accessToken');
      }
    }
  }

  return {
    // Read-only state access.
    get user() {
      return state.user;
    },
    get isLoggedIn() {
      return state.isLoggedIn;
    },
    get accessToken() {
      return state.accessToken;
    },
    get isLoading() {
      return state.isLoading;
    },

    setUser(user: User, accessToken: string): void {
      state.user = user;
      state.accessToken = accessToken;
      state.isLoggedIn = true;

      if (typeof window !== 'undefined') {
        localStorage.setItem('user', JSON.stringify(user));
        localStorage.setItem('accessToken', accessToken);
      }
    },

    updateUser(updates: Partial<User>): void {
      if (state.user) {
        state.user = { ...state.user, ...updates };

        if (typeof window !== 'undefined') {
          localStorage.setItem('user', JSON.stringify(state.user));
        }
      }
    },

    clearUser(): void {
      state.user = null;
      state.accessToken = null;
      state.isLoggedIn = false;

      if (typeof window !== 'undefined') {
        localStorage.removeItem('user');
        localStorage.removeItem('accessToken');
        localStorage.removeItem('refreshToken');
      }
    },

    setLoading(loading: boolean): void {
      state.isLoading = loading;
    },

    hasValidToken(): boolean {
      return !!state.accessToken && state.isLoggedIn;
    }
  };
}

export const userStore = createUserStore();
