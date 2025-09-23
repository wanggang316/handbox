<script lang="ts">
  import { chatState, chatActions } from '$lib/states/chat.svelte';
  import Button from '../../ui/Button.svelte';
  import TableGroup from '../../ui/table/TableGroup.svelte';
  import SwitchRow from '../../ui/table/SwitchRow.svelte';
  import RoundButton from '../../ui/RoundButton.svelte';
  import { Server, RefreshCw } from '@lucide/svelte';

  interface McpServer {
    id: string;
    name: string;
    enabled: boolean;
    description?: string;
    status?: 'enabled' | 'disabled' | 'error';
  }

  // 预定义的 MCP 服务器列表 (后续可以从后端获取)
  const availableMcpServers = [
    {
      id: 'filesystem',
      name: 'File System',
      description: '文件系统操作工具，可以读取、写入和管理本地文件',
      enabled: false
    },
    {
      id: 'browser',
      name: 'Browser Automation',
      description: '浏览器自动化工具，支持网页抓取和自动化操作',
      enabled: false
    },
    {
      id: 'database',
      name: 'Database Query',
      description: '数据库查询工具，支持 SQL 查询和数据分析',
      enabled: false
    },
    {
      id: 'github',
      name: 'GitHub Integration',
      description: 'GitHub API 集成，可以管理代码仓库和问题',
      enabled: false
    },
    {
      id: 'calendar',
      name: 'Calendar',
      description: '日历管理工具，可以查看和管理日程安排',
      enabled: false
    }
  ];

  // 从当前聊天获取已启用的 MCP 服务器
  const getEnabledServers = (): McpServer[] => {
    const enabledIds = chatState.currentChat?.mcpServers || [];
    return availableMcpServers.map(server => ({
      ...server,
      enabled: enabledIds.includes(server.id),
      status: (enabledIds.includes(server.id) ? 'enabled' : 'disabled') as 'enabled' | 'disabled'
    }));
  };

  let currentServers = $state<McpServer[]>(getEnabledServers());
  let originalEnabledServers = $state<string[]>(chatState.currentChat?.mcpServers || []);

  // 监听 currentChat 变化，更新本地状态
  $effect(() => {
    currentServers = getEnabledServers();
    originalEnabledServers = chatState.currentChat?.mcpServers || [];
  });

  let hasChanges = $derived(() => {
    const currentEnabledIds = currentServers
      .filter(s => s.enabled)
      .map(s => s.id)
      .sort();
    const originalIds = originalEnabledServers.sort();
    return JSON.stringify(currentEnabledIds) !== JSON.stringify(originalIds);
  });

  async function handleSave() {
    try {
      const enabledServerIds = currentServers
        .filter(s => s.enabled)
        .map(s => s.id);

      await chatActions.updateMcpServers(enabledServerIds);

      // 更新原始状态
      originalEnabledServers = enabledServerIds;
    } catch (error) {
      console.error('Failed to update MCP servers:', error);
      // 回滚到原始状态
      currentServers = getEnabledServers();
    }
  }

  function handleReset() {
    currentServers = getEnabledServers();
  }

  function handleRefresh() {
    // 暂时只是刷新本地状态，后续可以调用后端 API 获取最新状态
    currentServers = [...currentServers];
  }
</script>

<div class="flex-1 mt-1 p-0 space-y-2">
  <div class="flex items-center justify-between">
    <div class="text-sm text-base-content/70">
      {#if chatState.currentChat}
        已启用 {currentServers.filter(s => s.enabled).length} 个服务器
      {:else}
        请先选择或创建聊天
      {/if}
    </div>

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
  {#if currentServers.length > 0 && chatState.currentChat}
    <TableGroup>
      {#each currentServers as server (server.id)}
        <SwitchRow
          label={server.name}
          description={server.description}
          bind:checked={server.enabled}
        />
      {/each}
    </TableGroup>

    <!-- 操作按钮 -->
    <div class="flex gap-3 pt-4 justify-end">
      <RoundButton
        customClass="w-24"
        label="重置"
        bgColor="bg-base-200"
        textColor="text-base-content/80"
        hoverColor="hover:text-base-content"
        onclick={handleReset}
        disabled={!hasChanges()}
      />

      <RoundButton
        customClass="w-18"
        label="保存"
        onclick={handleSave}
        disabled={!hasChanges()}
      />
    </div>
  {:else if !chatState.currentChat}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">请先选择或创建聊天</p>
      <p class="text-sm">MCP 服务器配置将与聊天关联</p>
    </div>
  {:else}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">暂无可用的 MCP 服务器</p>
      <p class="text-sm">请在应用设置中配置 MCP 服务器</p>
    </div>
  {/if}
</div>
