import type {
  CreateMcpServerRequest,
  McpServer,
  RefreshMcpServerRequest,
  ToggleMcpServerRequest,
  UpdateMcpServerRequest
} from '../types';
import * as mcpApi from '../api/mcp';
import { listen, emit, type UnlistenFn } from '@tauri-apps/api/event';

declare global {
  interface Window {
    __TAURI__?: unknown;
    isTauri?: boolean;
  }
}

/**
 * 检测是否在 Tauri 环境中运行
 */
function isTauriEnvironment(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  // Tauri 2.0+ 推荐方式
  if ('isTauri' in window && window.isTauri === true) {
    return true;
  }

  // 兼容旧版本
  if ('__TAURI__' in window && window.__TAURI__) {
    return true;
  }

  return false;
}

/**
 * 使用 Tauri 2 的 emit() API 向所有窗口广播 MCP 更新事件
 */
async function emitMcpServersUpdated(
  payload: Record<string, unknown>
): Promise<void> {
  console.log('[emitMcpServersUpdated] Checking environment...');
  console.log('[emitMcpServersUpdated] isTauriEnvironment:', isTauriEnvironment());

  if (!isTauriEnvironment()) {
    console.log('[emitMcpServersUpdated] Not in Tauri environment, skipping emit');
    return;
  }

  try {
    console.log('[emitMcpServersUpdated] Emitting mcp-servers:updated event with payload:', payload);
    // Tauri 2: emit() 自动广播到所有窗口
    await emit('mcp-servers:updated', payload);
    console.log('[emitMcpServersUpdated] Event emitted successfully to all windows');
  } catch (error) {
    console.error('[emitMcpServersUpdated] Failed to broadcast mcp-servers:updated event:', error);
  }
}

/**
 * 标记 MCP 服务器已更新，并广播事件
 */
function markMcpServersDirty(
  reason: string,
  data?: Record<string, unknown>
): void {
  console.log('[markMcpServersDirty] Called with reason:', reason, 'data:', data);
  const payload = data ? { reason, ...data } : { reason };
  console.log('[markMcpServersDirty] Calling emitMcpServersUpdated with payload:', payload);
  void emitMcpServersUpdated(payload);
}

let mcpServersUpdatedUnlisten: UnlistenFn | null = null;

interface McpStateData {
  servers: McpServer[];
  isLoading: boolean;
  error: string | null;
  initialized: boolean;
  needsRefresh: boolean;
}

class McpState {
  private state = $state<McpStateData>({
    servers: [],
    isLoading: false,
    error: null,
    initialized: false,
    needsRefresh: false
  });

  get servers(): McpServer[] {
    return this.state.servers;
  }

  get isLoading(): boolean {
    return this.state.isLoading;
  }

  get error(): string | null {
    return this.state.error;
  }

  get initialized(): boolean {
    return this.state.initialized;
  }

  get needsRefresh(): boolean {
    return this.state.needsRefresh;
  }

  markNeedsRefresh(): void {
    this.state.needsRefresh = true;
  }

  clearNeedsRefresh(): void {
    this.state.needsRefresh = false;
  }

  private setLoading(value: boolean) {
    this.state.isLoading = value;
  }

  private setError(message: string | null) {
    this.state.error = message;
  }

  private setServers(servers: McpServer[]) {
    this.state.servers = servers;
  }

  async loadServers(force = false): Promise<void> {
    if (this.state.isLoading) return;
    if (this.state.initialized && !force) return;

    try {
      this.setLoading(true);
      this.setError(null);
      const servers = await mcpApi.listMcpServers();
      this.setServers(servers);
      this.state.initialized = true;
      this.clearNeedsRefresh();
    } catch (error) {
      const message = error instanceof Error ? error.message : '加载 MCP 服务器失败';
      this.setError(message);
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  async createServer(request: CreateMcpServerRequest): Promise<McpServer> {
    try {
      const server = await mcpApi.createMcpServer(request);
      this.setServers([server, ...this.state.servers]);
      markMcpServersDirty('mcp-server-created', { serverId: server.id });
      return server;
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '创建 MCP 服务器失败');
      throw error;
    }
  }

  async updateServer(serverId: string, request: UpdateMcpServerRequest): Promise<McpServer> {
    try {
      const updated = await mcpApi.updateMcpServer(serverId, request);
      this.state.servers = this.state.servers.map(server =>
        server.id === updated.id ? updated : server
      );
      markMcpServersDirty('mcp-server-updated', { serverId });
      return updated;
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '更新 MCP 服务器失败');
      throw error;
    }
  }

  async deleteServer(serverId: string): Promise<void> {
    try {
      await mcpApi.deleteMcpServer(serverId);
      this.state.servers = this.state.servers.filter(server => server.id !== serverId);
      markMcpServersDirty('mcp-server-deleted', { serverId });
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '删除 MCP 服务器失败');
      throw error;
    }
  }

  async toggleServer(request: ToggleMcpServerRequest): Promise<McpServer> {
    try {
      const updated = await mcpApi.toggleMcpServer(request);
      this.state.servers = this.state.servers.map(server =>
        server.id === updated.id ? updated : server
      );
      markMcpServersDirty('mcp-server-toggled', { serverId: request.serverId, enabled: request.enabled });
      return updated;
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '切换 MCP 服务器状态失败');
      throw error;
    }
  }

  async refreshServer(request: RefreshMcpServerRequest): Promise<McpServer> {
    try {
      const refreshed = await mcpApi.refreshMcpServer(request);
      this.state.servers = this.state.servers.map(server =>
        server.id === refreshed.id ? refreshed : server
      );
      markMcpServersDirty('mcp-server-refreshed', { serverId: request.serverId });
      return refreshed;
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '刷新 MCP 服务器失败');
      throw error;
    }
  }

  getServerById(id: string): McpServer | undefined {
    return this.state.servers.find(server => server.id === id);
  }
}

export const mcpState = new McpState();

export const mcpActions = {
  loadServers: (force = false) => mcpState.loadServers(force),
  createServer: (request: CreateMcpServerRequest) => mcpState.createServer(request),
  updateServer: (serverId: string, request: UpdateMcpServerRequest) =>
    mcpState.updateServer(serverId, request),
  deleteServer: (serverId: string) => mcpState.deleteServer(serverId),
  toggleServer: (request: ToggleMcpServerRequest) => mcpState.toggleServer(request),
  refreshServer: (request: RefreshMcpServerRequest) => mcpState.refreshServer(request),
  // 手动触发 MCP 更新通知（用于不通过 mcpActions 的操作）
  notifyMcpServersUpdated: (reason: string, data?: Record<string, unknown>) => {
    markMcpServersDirty(reason, data);
  }
};

/**
 * 注册 mcp-servers:updated 事件监听器
 * 应该在组件 onMount 时调用，确保 Tauri 环境已准备好
 */
export async function setupMcpServersUpdatedListener(): Promise<void> {
  console.log('[setupMcpServersUpdatedListener] Setting up listener...');
  console.log('[setupMcpServersUpdatedListener] Environment check:');
  console.log('  - typeof window:', typeof window);
  console.log('  - window.isTauri:', typeof window !== 'undefined' ? window.isTauri : 'N/A');
  console.log('  - window.__TAURI__:', typeof window !== 'undefined' ? window.__TAURI__ : 'N/A');
  console.log('  - isTauriEnvironment():', isTauriEnvironment());
  console.log('[setupMcpServersUpdatedListener] mcpServersUpdatedUnlisten:', mcpServersUpdatedUnlisten);

  if (!isTauriEnvironment()) {
    console.warn('[setupMcpServersUpdatedListener] ⚠️  Not in Tauri environment!');
    console.warn('  Make sure you are running "npm run tauri dev", not just "npm run dev"');
    console.warn('  Cross-window events will not work in browser-only mode');
    return;
  }

  if (mcpServersUpdatedUnlisten) {
    console.log('[setupMcpServersUpdatedListener] Listener already set up');
    return;
  }

  try {
    console.log('[setupMcpServersUpdatedListener] Registering listener for mcp-servers:updated event');
    mcpServersUpdatedUnlisten = await listen('mcp-servers:updated', event => {
      console.log('[mcpServersUpdatedListener] mcp-servers:updated event received', event);
      // 标记需要刷新，让组件响应式地检测并加载
      mcpState.markNeedsRefresh();
      console.log('[mcpServersUpdatedListener] Set needsRefresh to true');
    });
    console.log('[setupMcpServersUpdatedListener] Listener registered successfully');
  } catch (error) {
    console.error('[setupMcpServersUpdatedListener] Failed to register mcp-servers:updated listener:', error);
  }
}

/**
 * 清理事件监听器
 */
export function cleanupMcpServersUpdatedListener(): void {
  if (mcpServersUpdatedUnlisten) {
    console.log('[cleanupMcpServersUpdatedListener] Cleaning up listener');
    mcpServersUpdatedUnlisten();
    mcpServersUpdatedUnlisten = null;
  }
}
