import type { Timestamp } from './index';

export type McpServerStatus = 'inactive' | 'ready' | 'error' | 'unknown';

export type McpConnectionType = 'stdio' | 'sse' | 'http';

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: unknown;
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
  lastSyncAt?: Timestamp;
  lastError?: string;
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
