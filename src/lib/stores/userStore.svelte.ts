/**
 * 用户状态管理 Store
 *
 * 使用 Svelte 5 的 $state 和 $derived 语法管理全局用户状态
 */

import type { User, UserState } from '$lib/types/user';

/**
 * 创建用户状态管理
 */
function createUserStore() {
  // 使用 $state 创建响应式状态
  let state = $state<UserState>({
    user: null,
    isLoggedIn: false,
    accessToken: null,
    isLoading: false
  });

  // 从 localStorage 恢复用户状态（仅在浏览器环境）
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
        // 清除无效数据
        localStorage.removeItem('user');
        localStorage.removeItem('accessToken');
      }
    }
  }

  return {
    // 暴露只读的状态访问
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

    /**
     * 设置用户登录状态
     */
    setUser(user: User, accessToken: string): void {
      state.user = user;
      state.accessToken = accessToken;
      state.isLoggedIn = true;

      // 持久化到 localStorage
      if (typeof window !== 'undefined') {
        localStorage.setItem('user', JSON.stringify(user));
        localStorage.setItem('accessToken', accessToken);
      }
    },

    /**
     * 更新用户信息
     */
    updateUser(updates: Partial<User>): void {
      if (state.user) {
        state.user = { ...state.user, ...updates };

        // 更新 localStorage
        if (typeof window !== 'undefined') {
          localStorage.setItem('user', JSON.stringify(state.user));
        }
      }
    },

    /**
     * 清除用户登录状态
     */
    clearUser(): void {
      state.user = null;
      state.accessToken = null;
      state.isLoggedIn = false;

      // 清除 localStorage
      if (typeof window !== 'undefined') {
        localStorage.removeItem('user');
        localStorage.removeItem('accessToken');
        localStorage.removeItem('refreshToken');
      }
    },

    /**
     * 设置加载状态
     */
    setLoading(loading: boolean): void {
      state.isLoading = loading;
    },

    /**
     * 检查令牌是否有效
     */
    hasValidToken(): boolean {
      return !!state.accessToken && state.isLoggedIn;
    }
  };
}

// 导出单例实例
export const userStore = createUserStore();
