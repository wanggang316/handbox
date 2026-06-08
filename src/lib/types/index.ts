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
export type ApiResponse<T> =
  | {
      success: true;
      data: T;
    }
  | {
      success: false;
      error: AppError;
    };

// 基础实体接口
export interface BaseEntity {
  id?: UUID; // 可以是 undefined，表示还没有保存到后端
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

// 导出子模块类型
export * from "./agent";
export * from "./chat";
export * from "./provider";
export * from "./artifact";
export * from "./settings";
export * from "./mcp";
export * from "./user";
export * from "./word";
export * from "./agentSession";
