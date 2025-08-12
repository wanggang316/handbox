/**
 * 全局类型定义
 */

// 基础类型
export type UUID = string;
export type Timestamp = number;

// API 错误响应结构
export interface AppError {
  code: string;
  message: string;
  hint?: string;
}

// API 响应包装类型
export type ApiResponse<T> = {
  success: true;
  data: T;
} | {
  success: false;
  error: AppError;
};

// 基础实体接口
export interface BaseEntity {
  id: UUID;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

// 导出子模块类型
export * from './chat';
export * from './provider';
export * from './artifact';
export * from './settings';