/**
 * API 统一封装层
 * 
 * 提供对 Tauri IPC 命令的封装，统一错误处理和类型安全
 */

import { invoke } from '@tauri-apps/api/core';
import type { ApiResponse } from '../types';
import type { AppError as AppErrorType } from '../types';

/**
 * 统一的 IPC 调用封装
 */
export async function apiCall<T>(
  command: string,
  payload?: unknown
): Promise<T> {
  try {
    const result = await invoke<ApiResponse<T>>(command, payload as Record<string, unknown>);
    
    if (result.success) {
      return result.data;
    } else {
      throw new AppError(result.error.code, result.error.message, result.error.hint);
    }
  } catch (error) {
    if (error instanceof AppError) {
      throw error;
    }
    
    // 处理 Tauri 调用错误
    throw new AppError(
      'IPC_ERROR',
      error instanceof Error ? error.message : 'IPC 调用失败',
      '请检查应用状态或重新启动'
    );
  }
}

/**
 * 自定义错误类
 */
export class AppError extends Error {
  constructor(
    public code: string,
    message: string,
    public hint?: string
  ) {
    super(message);
    this.name = 'AppError';
  }
}

// 导出 API 模块
export * from './chat';
export * from './provider';
export * from './artifact';
export * from './settings';
export * from './search';