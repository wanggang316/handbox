import { AppError as ApiAppError } from '$lib/api';
import type { AppError as AppErrorShape } from '$lib/types';
import { toastActions, type ToastOptions } from '$lib/states/toast.svelte';

export interface ErrorDisplayOptions {
  requiresAcknowledgement?: boolean;
  duration?: number;
  hint?: string;
  fallbackMessage?: string;
  acknowledgeLabel?: string;
  title?: string;
  code?: string;
  toastOptions?: ToastOptions;
}

function isAppErrorShape(value: unknown): value is AppErrorShape {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<AppErrorShape>;
  return typeof candidate.code === 'string' && typeof candidate.message === 'string';
}

export function normalizeError(
  error: unknown,
  fallbackMessage = '操作失败，请稍后重试'
): AppErrorShape {
  if (error instanceof ApiAppError) {
    return {
      code: error.code,
      message: error.message,
      hint: error.hint
    };
  }

  if (isAppErrorShape(error)) {
    return {
      code: error.code,
      message: error.message,
      hint: error.hint
    };
  }

  if (error instanceof Error) {
    return {
      code: error.name || 'UNEXPECTED_ERROR',
      message: error.message || fallbackMessage,
      hint: undefined
    };
  }

  if (typeof error === 'string') {
    return {
      code: 'UNKNOWN_ERROR',
      message: error || fallbackMessage,
      hint: undefined
    };
  }

  return {
    code: 'UNKNOWN_ERROR',
    message: fallbackMessage,
    hint: undefined
  };
}

export function showAppError(
  error: unknown,
  options?: ErrorDisplayOptions
): AppErrorShape {
  const normalized = normalizeError(error, options?.fallbackMessage);

  const toastOptions: ToastOptions = {
    requiresAcknowledgement: options?.requiresAcknowledgement,
    duration: options?.duration,
    hint: options?.hint ?? normalized.hint,
    acknowledgeLabel: options?.acknowledgeLabel,
    title: options?.title,
    code: options?.code ?? normalized.code,
    ...(options?.toastOptions ?? {})
  };

  toastActions.error(normalized.message, toastOptions);

  return normalized;
}
