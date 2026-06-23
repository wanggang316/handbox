/**
 * API 统一封装层
 *
 * 提供对 Tauri IPC 命令的封装，统一错误处理和类型安全
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * 统一的 IPC 调用封装
 */
export async function apiCall<T>(
  command: string,
  payload?: unknown,
): Promise<T> {
  try {
    // Tauri 命令直接返回数据类型 T，错误会被抛出
    return await invoke<T>(command, payload as Record<string, unknown>);
  } catch (error: any) {
    // 处理 Tauri IPC 错误
    if (error && typeof error === "object") {
      // 如果错误包含我们的 AppError 结构
      if (error.code && error.message) {
        throw new AppError(error.code, error.message, error.hint);
      }
      // 如果是包装在其他结构中的错误
      if (error.error && error.error.code && error.error.message) {
        throw new AppError(
          error.error.code,
          error.error.message,
          error.error.hint,
        );
      }
    }

    // 处理其他类型的错误
    throw new AppError(
      "IPC_ERROR",
      error instanceof Error
        ? error.message
        : typeof error === "string"
          ? error
          : "IPC 调用失败",
      "请检查应用状态或重新启动",
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
    public hint?: string,
  ) {
    super(message);
    this.name = "AppError";
  }
}

// 导出 API 模块
export * from "./accessibility";
export * from "./agent";
export * from "./genui";
export * from "./chat";
export * from "./message";
export * from "./provider";
export * from "./model";
export * from "./artifact";
export * from "./settings";
export * from "./window";
export * from "./mcp";
export * from "./skill";
export * from "./auth";
export * from "./word";
export * from "./favorite";
export * from "./selection";
export * from "./agentSession";
export * from "./agentProject";
export * from "./openIn";
export * from "./job";
