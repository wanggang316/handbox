import { invoke } from "@tauri-apps/api/core";

/** Invokes a Tauri command, normalizing thrown errors into AppError. */
export async function apiCall<T>(
  command: string,
  payload?: unknown,
): Promise<T> {
  try {
    return await invoke<T>(command, payload as Record<string, unknown>);
  } catch (error: any) {
    if (error && typeof error === "object") {
      // Direct AppError payload.
      if (error.code && error.message) {
        throw new AppError(error.code, error.message, error.hint);
      }
      // AppError wrapped in an outer structure.
      if (error.error && error.error.code && error.error.message) {
        throw new AppError(
          error.error.code,
          error.error.message,
          error.error.hint,
        );
      }
    }

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

export * from "./accessibility";
export * from "./agent";
export * from "./genui";
export * from "./provider";
export * from "./model";
export * from "./settings";
export * from "./window";
export * from "./mcp";
export * from "./hookRule";
export * from "./skill";
export * from "./auth";
export * from "./selection";
export * from "./agentSession";
export * from "./agentProject";
export * from "./openIn";
export * from "./job";
