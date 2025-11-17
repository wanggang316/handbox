/**
 * UI 状态管理 - Svelte 5
 */

import type { Theme, ThemeColor, Language } from '../types';

// 通知消息接口
export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message?: string;
  duration?: number;
  actions?: Array<{ label: string; action: () => void }>;
}

interface UIStateData {
  sidebarOpen: boolean;
  currentPage: string;
  modals: Record<string, boolean>;
  notifications: Notification[];
  theme: Theme;
  themeColor: ThemeColor;
  language: Language;
  globalLoading: boolean;
}

class UIState {
  private state = $state<UIStateData>({
    sidebarOpen: true,
    currentPage: 'chat',
    modals: {},
    notifications: [],
    theme: 'system',
    themeColor: 'blue',
    language: 'zh-CN',
    globalLoading: false,
  });

  // Getters
  get sidebarOpen() {
    return this.state.sidebarOpen;
  }

  get currentPage() {
    return this.state.currentPage;
  }

  get modals() {
    return this.state.modals;
  }

  get notifications() {
    return this.state.notifications;
  }

  get theme() {
    return this.state.theme;
  }

  get themeColor() {
    return this.state.themeColor;
  }

  get language() {
    return this.state.language;
  }

  get globalLoading() {
    return this.state.globalLoading;
  }

  // 派生状态：是否为暗色主题
  get isDarkMode(): boolean {
    const theme = this.state.theme;
    if (theme === 'dark') return true;
    if (theme === 'light') return false;
    
    // 跟随系统 - 需要检查浏览器环境
    if (typeof window !== 'undefined') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches;
    }
    return false;
  }

  // Actions
  setSidebarOpen(open: boolean) {
    this.state.sidebarOpen = open;
  }

  setCurrentPage(page: string) {
    this.state.currentPage = page;
  }

  setModals(modals: Record<string, boolean>) {
    this.state.modals = modals;
  }

  setNotifications(notifications: Notification[]) {
    this.state.notifications = notifications;
  }

  setThemeState(theme: Theme) {
    this.state.theme = theme;
  }

  setThemeColorState(color: ThemeColor) {
    this.state.themeColor = color;
  }

  setLanguageState(language: Language) {
    this.state.language = language;
  }

  setGlobalLoading(loading: boolean) {
    this.state.globalLoading = loading;
  }

  /**
   * 切换侧边栏
   */
  toggleSidebar(): void {
    this.state.sidebarOpen = !this.state.sidebarOpen;
  }

  /**
   * 打开模态框
   */
  openModal(modalId: string): void {
    this.state.modals = { ...this.state.modals, [modalId]: true };
  }

  /**
   * 关闭模态框
   */
  closeModal(modalId: string): void {
    this.state.modals = { ...this.state.modals, [modalId]: false };
  }

  /**
   * 切换模态框状态
   */
  toggleModal(modalId: string): void {
    this.state.modals = { 
      ...this.state.modals, 
      [modalId]: !this.state.modals[modalId] 
    };
  }

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

    this.state.notifications = [...this.state.notifications, newNotification];

    // 自动移除通知
    if (newNotification.duration && newNotification.duration > 0) {
      setTimeout(() => {
        this.removeNotification(id);
      }, newNotification.duration);
    }

    return id;
  }

  /**
   * 移除通知
   */
  removeNotification(id: string): void {
    this.state.notifications = this.state.notifications.filter(n => n.id !== id);
  }

  /**
   * 清空所有通知
   */
  clearNotifications(): void {
    this.state.notifications = [];
  }

  /**
   * 设置主题
   */
  setTheme(newTheme: Theme): void {
    this.state.theme = newTheme;
    
    // 保存到 localStorage
    if (typeof localStorage !== 'undefined') {
      const current = localStorage.getItem('theme');
      if (current !== newTheme) {
        localStorage.setItem('theme', newTheme);
      }
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
  }

  /**
   * 设置主题色
   */
  setThemeColor(color: ThemeColor): void {
    this.state.themeColor = color;
    
    if (typeof localStorage !== 'undefined') {
      const current = localStorage.getItem('themeColor');
      if (current !== color) {
        localStorage.setItem('themeColor', color);
      }
    }
    
    // 更新 CSS 变量
    if (typeof document !== 'undefined') {
      document.documentElement.style.setProperty('--theme-color', color);
    }
  }

  /**
   * 设置语言
   */
  setLanguage(lang: Language): void {
    this.state.language = lang;
    
    // 更新 HTML lang 属性
    if (typeof document !== 'undefined') {
      document.documentElement.lang = lang;
    }
  }
}

// 导出单例实例
export const uiState = new UIState();
