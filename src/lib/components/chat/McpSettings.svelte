<script lang="ts">
  import Toggle from '../ui/Toggle.svelte';
  import Button from '../ui/Button.svelte';
  import { Server, Save, RefreshCw, AlertCircle, CheckCircle } from '@lucide/svelte';

  interface McpServer {
    id: string;
    name: string;
    status: 'enabled' | 'disabled' | 'error';
    enabled: boolean;
    description?: string;
    command?: string;
  }

  interface Props {
    servers?: McpServer[];
    onSave?: (enabledServers: string[]) => void;
    onRefresh?: () => void;
  }

  let { 
    servers = [],
    onSave,
    onRefresh
  }: Props = $props();

  // 示例 MCP 服务器数据
  let currentServers = $state<McpServer[]>(servers.length > 0 ? servers : [
    {
      id: 'filesystem',
      name: 'File System',
      status: 'enabled',
      enabled: true,
      description: '文件系统操作工具，可以读取、写入和管理本地文件',
      command: 'mcp-server-filesystem'
    },
    {
      id: 'browser',
      name: 'Browser Automation',
      status: 'disabled',
      enabled: false,
      description: '浏览器自动化工具，支持网页抓取和自动化操作',
      command: 'mcp-server-puppeteer'
    },
    {
      id: 'database',
      name: 'Database Query',
      status: 'error',
      enabled: false,
      description: '数据库查询工具，支持 SQL 查询和数据分析',
      command: 'mcp-server-sqlite'
    },
    {
      id: 'github',
      name: 'GitHub Integration',
      status: 'enabled',
      enabled: true,
      description: 'GitHub API 集成，可以管理代码仓库和问题',
      command: 'mcp-server-github'
    },
    {
      id: 'calendar',
      name: 'Calendar',
      status: 'disabled',
      enabled: false,
      description: '日历管理工具，可以查看和管理日程安排',
      command: 'mcp-server-calendar'
    }
  ]);

  const originalEnabledServers = servers.filter(s => s.enabled).map(s => s.id);
  let hasChanges = $derived(
    JSON.stringify(currentServers.filter(s => s.enabled).map(s => s.id).sort()) !== 
    JSON.stringify(originalEnabledServers.sort())
  );

  function handleToggleServer(serverId: string) {
    currentServers = currentServers.map(server => 
      server.id === serverId 
        ? { ...server, enabled: !server.enabled }
        : server
    );
  }

  function handleSave() {
    const enabledServerIds = currentServers.filter(s => s.enabled).map(s => s.id);
    onSave?.(enabledServerIds);
  }

  function handleRefresh() {
    onRefresh?.();
  }



  function getStatusText(status: string) {
    switch (status) {
      case 'enabled':
        return '已连接';
      case 'error':
        return '连接失败';
      default:
        return '未连接';
    }
  }
</script>

<div class="flex-1 p-6 space-y-6">
  <!-- 标题 -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <Server size={20} />
      <h3 class="text-lg font-medium text-gray-900">MCP 服务器</h3>
    </div>
    <Button
      on:click={handleRefresh}
      variant="secondary"
      size="sm"
    >
      <RefreshCw size={14} />
      刷新状态
    </Button>
  </div>

  <!-- 说明 -->
  <div class="text-sm text-gray-600 bg-blue-50 p-4 rounded-lg">
    <p class="mb-2">MCP (Model Context Protocol) 服务器为AI提供额外的工具和能力。</p>
    <p>启用的服务器将在此聊天会话中可用，AI可以调用这些工具来完成复杂任务。</p>
  </div>

  <!-- 服务器列表 -->
  <div class="space-y-4">
    {#each currentServers as server (server.id)}
      <div class="border border-gray-200 rounded-lg p-4 space-y-3">
        <!-- 服务器头部信息 -->
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            {#if server.status === 'enabled'}
              <CheckCircle size={16} class="text-green-500" />
            {:else if server.status === 'error'}
              <AlertCircle size={16} class="text-red-500" />
            {:else}
              <Server size={16} class="text-gray-400" />
            {/if}
            <div>
              <h4 class="font-medium text-gray-900">{server.name}</h4>
              <div class="flex items-center gap-2 text-xs text-gray-500">
                <span>{getStatusText(server.status)}</span>
                {#if server.command}
                  <span>•</span>
                  <code class="bg-gray-100 px-1 py-0.5 rounded">{server.command}</code>
                {/if}
              </div>
            </div>
          </div>
          <Toggle 
            checked={server.enabled} 
            onChange={() => handleToggleServer(server.id)}
          />
        </div>

        <!-- 服务器描述 -->
        {#if server.description}
          <p class="text-sm text-gray-600 pl-7">{server.description}</p>
        {/if}

        <!-- 错误状态提示 -->
        {#if server.status === 'error'}
          <div class="pl-7">
            <div class="text-xs text-red-600 bg-red-50 p-2 rounded">
              <AlertCircle size={12} class="inline mr-1" />
              服务器连接失败，请检查配置或重新启动服务器
            </div>
          </div>
        {/if}
      </div>
    {/each}

    {#if currentServers.length === 0}
      <div class="text-center py-8 text-gray-500">
        <Server size={48} class="mx-auto mb-4 text-gray-300" />
        <p class="mb-2">暂无 MCP 服务器</p>
        <p class="text-sm">请在应用设置中配置 MCP 服务器</p>
      </div>
    {/if}
  </div>

  <!-- 操作按钮 -->
  <div class="flex justify-between items-center pt-4 border-t border-gray-200">
    <div class="text-sm text-gray-500">
      已启用 {currentServers.filter(s => s.enabled).length} 个服务器
    </div>
    <Button
      on:click={handleSave}
      variant="primary"
      disabled={!hasChanges}
    >
      <Save size={14} />
      保存设置
    </Button>
  </div>
</div>
