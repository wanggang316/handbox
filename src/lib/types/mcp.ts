import type { Timestamp } from './index';

export type McpServerStatus = 'inactive' | 'ready' | 'error' | 'unknown';

export type McpConnectionType = 'stdio' | 'sse' | 'http';

export type ToolExecutionMode = 'auto' | 'manual';

export type McpErrorType =
  | 'connection_error'
  | 'authentication_error'
  | 'timeout_error'
  | 'configuration_error'
  | 'protocol_error'
  | 'unknown_error';

export interface McpErrorDetail {
  errorType: McpErrorType;
  message: string;
  details?: string;
  timestamp: Timestamp;
}

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: unknown;
  annotations?: Record<string, unknown>;
}

export interface McpPromptArgument {
  name: string;
  description?: string;
  required?: boolean;
}

export interface McpPrompt {
  name: string;
  description?: string;
  arguments: McpPromptArgument[];
}

export interface McpResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  annotations?: Record<string, unknown>;
}

export interface McpServer {
  id: string;
  name: string;
  displayName?: string;
  description?: string;
  connectionType: McpConnectionType;
  command: string;
  args: string[];
  workingDir?: string;
  env: Record<string, string>;
  endpoint?: string;
  headers: Record<string, string>;
  timeoutMs?: number;
  enabled: boolean;
  status: McpServerStatus;
  tools: McpTool[];
  prompts: McpPrompt[];
  resources: McpResource[];
  enabledTools: string[];
  toolExecutionMode: Record<string, ToolExecutionMode>;
  lastSyncAt?: Timestamp;
  lastError?: McpErrorDetail;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

export interface CreateMcpServerRequest {
  name: string;
  displayName?: string;
  description?: string;
  connectionType?: McpConnectionType;
  command: string;
  args?: string[];
  workingDir?: string;
  env?: Record<string, string>;
  endpoint?: string;
  headers?: Record<string, string>;
  timeoutMs?: number;
  enabled?: boolean;
}

export interface UpdateMcpServerRequest {
  name?: string;
  displayName?: string;
  description?: string;
  connectionType?: McpConnectionType;
  command?: string;
  args?: string[];
  workingDir?: string;
  env?: Record<string, string>;
  endpoint?: string;
  headers?: Record<string, string>;
  timeoutMs?: number;
  enabled?: boolean;
}

export interface ToggleMcpServerRequest {
  serverId: string;
  enabled: boolean;
}

export interface RefreshMcpServerRequest {
  serverId: string;
}

export interface UpdateToolEnabledRequest {
  serverId: string;
  toolName: string;
  enabled: boolean;
}

export interface UpdateToolExecutionModeRequest {
  serverId: string;
  toolName: string;
  executionMode: ToolExecutionMode;
}
