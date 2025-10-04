import { apiCall } from './index';
import type {
  CreateMcpServerRequest,
  McpServer,
  RefreshMcpServerRequest,
  ToggleMcpServerRequest,
  UpdateMcpServerRequest,
  UpdateToolEnabledRequest
} from '../types';

export async function listMcpServers(): Promise<McpServer[]> {
  return apiCall<McpServer[]>('mcp_list_servers');
}

export async function createMcpServer(request: CreateMcpServerRequest): Promise<McpServer> {
  return apiCall<McpServer>('mcp_create_server', { request });
}

export async function updateMcpServer(
  serverId: string,
  request: UpdateMcpServerRequest
): Promise<McpServer> {
  return apiCall<McpServer>('mcp_update_server', { serverId: serverId, request });
}

export async function deleteMcpServer(serverId: string): Promise<void> {
  await apiCall<void>('mcp_delete_server', { server_id: serverId });
}

export async function toggleMcpServer(request: ToggleMcpServerRequest): Promise<McpServer> {
  return apiCall<McpServer>('mcp_toggle_server', { request });
}

export async function refreshMcpServer(request: RefreshMcpServerRequest): Promise<McpServer> {
  return apiCall<McpServer>('mcp_refresh_server', { request });
}

export async function updateToolEnabled(request: UpdateToolEnabledRequest): Promise<McpServer> {
  return apiCall<McpServer>('mcp_update_tool_enabled', { request });
}
