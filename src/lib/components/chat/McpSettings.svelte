<script lang="ts">
  import Button from '../ui/Button.svelte';
  import TableGroup from '../ui/table/TableGroup.svelte';
  import SwitchRow from '../ui/table/SwitchRow.svelte';
  import { Server, Save, RefreshCw } from '@lucide/svelte';

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

  function handleRefresh() {
    onRefresh?.();
  }

</script>

<div class="flex-1 mt-1 p-0 space-y-2">

  <div class="flex items-center justify-end">
    <Button
      on:click={handleRefresh}
      variant="clear"
      size="sm"
    >
      <RefreshCw size={14} />
      刷新状态
    </Button>
  </div>

  <!-- 服务器列表 -->
  {#if currentServers.length > 0}
    <TableGroup>
      {#each currentServers as server (server.id)}
        <SwitchRow 
          label={server.name}
          bind:checked={server.enabled}
        />
      {/each}
    </TableGroup>
  {:else}
    <div class="text-center py-8 text-gray-500">
      <Server size={48} class="mx-auto mb-4 text-gray-300" />
      <p class="mb-2">暂无 MCP 服务器</p>
      <p class="text-sm">请在应用设置中配置 MCP 服务器</p>
    </div>
  {/if}
</div>
