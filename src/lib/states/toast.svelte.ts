// Global toast state.

export interface ToastMessage {
  id: string;
  message: string;
  type: 'error' | 'success' | 'warning' | 'info';
  title?: string;
  duration?: number;
  hint?: string;
  code?: string;
  requiresAcknowledgement?: boolean;
  acknowledgeLabel?: string;
}

export interface ToastOptions {
  id?: string;
  duration?: number;
  title?: string;
  hint?: string;
  code?: string;
  requiresAcknowledgement?: boolean;
  acknowledgeLabel?: string;
}

let toastState = $state<{
  messages: ToastMessage[];
}>({
  messages: []
});

const defaultDurations: Record<ToastMessage['type'], number> = {
  error: 3000,
  success: 2500,
  warning: 3000,
  info: 3000
};

function createToastId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return Date.now().toString();
}

function createToast(
  type: ToastMessage['type'],
  message: string,
  options?: ToastOptions
): ToastMessage {
  const requiresAcknowledgement = options?.requiresAcknowledgement ?? false;
  const baseDuration =
    options?.duration ?? defaultDurations[type] ?? defaultDurations.info;

  return {
    id: options?.id ?? createToastId(),
    type,
    message,
    title: options?.title,
    hint: options?.hint,
    code: options?.code,
    requiresAcknowledgement,
    acknowledgeLabel: requiresAcknowledgement
      ? options?.acknowledgeLabel ?? '我知道了'
      : undefined,
    duration: requiresAcknowledgement ? undefined : baseDuration
  };
}

export const toastStore = {
  get messages() {
    return toastState.messages;
  }
};

export const toastActions = {
  error(message: string, options?: ToastOptions) {
    this.add(createToast('error', message, options));
  },

  success(message: string, options?: ToastOptions) {
    this.add(createToast('success', message, options));
  },

  warning(message: string, options?: ToastOptions) {
    this.add(createToast('warning', message, options));
  },

  info(message: string, options?: ToastOptions) {
    this.add(createToast('info', message, options));
  },

  add(toast: ToastMessage) {
    toastState.messages.push(toast);

    // Auto-dismiss.
    if (toast.duration && toast.duration > 0) {
      setTimeout(() => {
        this.remove(toast.id);
      }, toast.duration);
    }
  },

  remove(id: string) {
    const index = toastState.messages.findIndex(t => t.id === id);
    if (index > -1) {
      toastState.messages.splice(index, 1);
    }
  },

  clear() {
    toastState.messages = [];
  }
};
