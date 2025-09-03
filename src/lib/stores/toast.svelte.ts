// 全局 Toast 状态管理

export interface ToastMessage {
  id: string;
  message: string;
  type: 'error' | 'success' | 'warning' | 'info';
  duration?: number;
}

let toastState = $state<{
  messages: ToastMessage[];
}>({
  messages: []
});

export const toastStore = {
  get messages() {
    return toastState.messages;
  }
};

export const toastActions = {
  /**
   * 显示错误提示
   */
  error(message: string, duration = 5000) {
    this.add({
      id: Date.now().toString(),
      message,
      type: 'error',
      duration
    });
  },

  /**
   * 显示成功提示
   */
  success(message: string, duration = 3000) {
    this.add({
      id: Date.now().toString(),
      message,
      type: 'success',
      duration
    });
  },

  /**
   * 显示警告提示
   */
  warning(message: string, duration = 3000) {
    this.add({
      id: Date.now().toString(),
      message,
      type: 'warning',
      duration
    });
  },

  /**
   * 显示信息提示
   */
  info(message: string, duration = 3000) {
    this.add({
      id: Date.now().toString(),
      message,
      type: 'info',
      duration
    });
  },

  /**
   * 添加提示消息
   */
  add(toast: ToastMessage) {
    toastState.messages.push(toast);
    
    // 自动移除
    if (toast.duration && toast.duration > 0) {
      setTimeout(() => {
        this.remove(toast.id);
      }, toast.duration);
    }
  },

  /**
   * 移除提示消息
   */
  remove(id: string) {
    const index = toastState.messages.findIndex(t => t.id === id);
    if (index > -1) {
      toastState.messages.splice(index, 1);
    }
  },

  /**
   * 清除所有提示
   */
  clear() {
    toastState.messages = [];
  }
};