import type {
  CreateMcpServerRequest,
  McpServer,
  RefreshMcpServerRequest,
  ToggleMcpServerRequest,
  UpdateMcpServerRequest
} from '../types';
import * as mcpApi from '../api/mcp';

interface McpStateData {
  servers: McpServer[];
  isLoading: boolean;
  error: string | null;
  initialized: boolean;
}

class McpState {
  private state = $state<McpStateData>({
    servers: [],
    isLoading: false,
    error: null,
    initialized: false
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
  refreshServer: (request: RefreshMcpServerRequest) => mcpState.refreshServer(request)
};
