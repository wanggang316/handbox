export type UUID = string;
export type Timestamp = number;

export interface AppError {
  code: string;
  message: string;
  hint?: string;
}

export type ApiResponse<T> =
  | {
      success: true;
      data: T;
    }
  | {
      success: false;
      error: AppError;
    };

export interface BaseEntity {
  id?: UUID; // Undefined until persisted by the backend.
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

export * from "./agent";
export * from "./genui";
export * from "./llm";
export * from "./provider";
export * from "./settings";
export * from "./mcp";
export * from "./hookRule";
export * from "./skill";
export * from "./user";
export * from "./agentSession";
export * from "./agentProject";
export * from "./job";
