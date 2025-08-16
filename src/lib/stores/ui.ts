/**
 * UI 状态管理
 */

import { writable, derived } from 'svelte/store';
import type { Theme, ThemeColor, Language } from '../types';

// 侧边栏状态
export const sidebarOpen = writable(true);

// 当前活跃页面
export const currentPage = writable<string>('chat');

// 模态框状态
export const modals = writable<Record<string, boolean>>({});

// 通知消息
export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message?: string;
  duration?: number;
  actions?: Array<{ label: string; action: () => void }>;
}

export const notifications = writable<Notification[]>([]);

// 主题设置
export const theme = writable<Theme>('system');
export const themeColor = writable<ThemeColor>('blue');

// 语言设置
export const language = writable<Language>('zh-CN');

// 加载状态
export const globalLoading = writable(false);

// 派生状态：是否为暗色主题
export const isDarkMode = derived(
  theme,
  ($theme) => {
    if ($theme === 'dark') return true;
    if ($theme === 'light') return false;
    
    // 跟随系统 - 需要检查浏览器环境
    if (typeof window !== 'undefined') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches;
    }
    return false;
  }
);

/**
 * UI 操作
 */
export const uiActions = {
  /**
   * 切换侧边栏
   */
  toggleSidebar(): void {
    sidebarOpen.update(open => !open);
  },

  /**
   * 设置当前页面
   */
  setCurrentPage(page: string): void {
    currentPage.set(page);
  },

  /**
   * 打开模态框
   */
  openModal(modalId: string): void {
    modals.update(current => ({ ...current, [modalId]: true }));
  },

  /**
   * 关闭模态框
   */
  closeModal(modalId: string): void {
    modals.update(current => ({ ...current, [modalId]: false }));
  },

  /**
   * 切换模态框状态
   */
  toggleModal(modalId: string): void {
    modals.update(current => ({ 
      ...current, 
      [modalId]: !current[modalId] 
    }));
  },

  /**
   * 显示通知
   */
  showNotification(notification: Omit<Notification, 'id'>): string {
    const id = crypto.randomUUID();
    const newNotification: Notification = {
      id,
      duration: 5000,
      ...notification
    };

    notifications.update(current => [...current, newNotification]);

    // 自动移除通知
    if (newNotification.duration && newNotification.duration > 0) {
      setTimeout(() => {
        uiActions.removeNotification(id);
      }, newNotification.duration);
    }

    return id;
  },

  /**
   * 移除通知
   */
  removeNotification(id: string): void {
    notifications.update(current => current.filter(n => n.id !== id));
  },

  /**
   * 清空所有通知
   */
  clearNotifications(): void {
    notifications.set([]);
  },

  /**
   * 设置主题
   */
  setTheme(newTheme: Theme): void {
    theme.set(newTheme);
    
    // 保存到 localStorage
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('theme', newTheme);
    }
    
    // 更新 HTML data-theme 属性以匹配 CSS 选择器
    if (typeof document !== 'undefined') {
      if (newTheme === 'system') {
        // 跟随系统主题
        const systemIsDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        document.documentElement.setAttribute('data-theme', systemIsDark ? 'dark' : 'light');
      } else {
        document.documentElement.setAttribute('data-theme', newTheme);
      }
    }
  },

  /**
   * 设置主题色
   */
  setThemeColor(color: ThemeColor): void {
    themeColor.set(color);
    
    // 更新 CSS 变量
    document.documentElement.style.setProperty('--theme-color', color);
  },

  /**
   * 设置语言
   */
  setLanguage(lang: Language): void {
    language.set(lang);
    
    // 更新 HTML lang 属性
    document.documentElement.lang = lang;
  },

  /**
   * 设置全局加载状态
   */
  setGlobalLoading(loading: boolean): void {
    globalLoading.set(loading);
  }
};