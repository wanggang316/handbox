<script lang="ts">
  import { onMount } from 'svelte';
  import { Server, RefreshCw } from '@lucide/svelte';
  import TableGroup from '$lib/components/ui/table/TableGroup.svelte';
  import TableBaseRow from '$lib/components/ui/table/TableBaseRow.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Toggle from '$lib/components/ui/Toggle.svelte';
  import StatusLabel from '$lib/components/ui/StatusLabel.svelte';
  import { chatState, chatActions } from '$lib/states/chat.svelte';
  import { mcpState, mcpActions } from '$lib/states/mcp.svelte';
  import type { McpServer, McpServerStatus } from '$lib/types';

  let currentServers = $state<string[]>(chatState.currentChat?.mcpServers || []);
  let originalServers = $state<string[]>(chatState.currentChat?.mcpServers || []);
  let saving = $state(false);
  let refreshing = $state(false);

  onMount(() => {
    if (!mcpState.initialized) {
      mcpActions.loadServers().catch(error => {
        console.error('Failed to load MCP servers:', error);
      });
    }
  });

  $effect(() => {
    currentServers = chatState.currentChat?.mcpServers || [];
    originalServers = chatState.currentChat?.mcpServers || [];
  });

  const hasChanges = $derived(() => {
    const currentSorted = [...currentServers].sort();
    const originalSorted = [...originalServers].sort();
    return JSON.stringify(currentSorted) !== JSON.stringify(originalSorted);
  });

  const availableServers = $derived(() =>
    mcpState.servers.filter(server => server.enabled)
  );

  const decoratedServers = $derived(() =>
    availableServers().map(server => ({
      server,
      checked: currentServers.includes(server.id),
      statusInfo: mapStatus(server)
    }))
  );

  function toggleSelection(serverId: string, selected: boolean) {
    if (selected) {
      if (!currentServers.includes(serverId)) {
        currentServers = [...currentServers, serverId];
      }
    } else {
      currentServers = currentServers.filter(id => id !== serverId);
    }
  }

  async function handleSave() {
    if (!chatState.currentChat?.id) {
      if (chatState.currentChat) {
        chatState.currentChat.mcpServers = currentServers;
      }
      originalServers = [...currentServers];
      return;
    }

    saving = true;
    try {
      await chatActions.updateMcpServers(currentServers);
      originalServers = [...currentServers];
    } catch (error) {
      console.error('Failed to update MCP servers:', error);
      await chatActions.loadChats();
    } finally {
      saving = false;
    }
  }

  function handleReset() {
    currentServers = [...originalServers];
  }

  async function handleRefresh() {
    refreshing = true;
    try {
      await mcpActions.loadServers(true);
    } catch (error) {
      console.error('Failed to refresh MCP servers:', error);
    } finally {
      refreshing = false;
    }
  }

  function mapStatus(server: McpServer): { status: 'enabled' | 'disabled' | 'idle' | 'error'; text: string } {
    if (!server.enabled) {
      return { status: 'disabled', text: '未启用' };
    }
    switch (server.status as McpServerStatus) {
      case 'ready':
        return { status: 'enabled', text: '可用' };
      case 'error':
        return { status: 'error', text: '异常' };
      default:
        return { status: 'idle', text: '同步中' };
    }
  }
</script>

<div class="flex-1 mt-1 p-0 space-y-3">
  <div class="flex items-center justify-between">
    <div class="text-sm text-base-content/70">
      {#if !chatState.currentChat}
        请先选择或创建聊天
      {:else}
        已选择 {currentServers.length} 个服务器
      {/if}
    </div>

    <Button
      on:click={handleRefresh}
      variant="clear"
      size="sm"
      disabled={refreshing}
    >
      <RefreshCw class={refreshing ? 'animate-spin' : ''} size={14} />
      刷新列表
    </Button>
  </div>

  {#if !chatState.currentChat}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">请先选择或创建聊天</p>
      <p class="text-sm">MCP 服务器配置将与聊天关联</p>
    </div>
  {:else if mcpState.servers.length === 0}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">暂无可用的 MCP 服务器</p>
      <p class="text-sm">请在应用设置中配置 MCP 服务器</p>
    </div>
  {:else}
    <TableGroup>
      {#each decoratedServers() as item (item.server.id)}
        <TableBaseRow label={item.server.displayName ?? item.server.name} layout="vertical">
            <div class="flex flex-col gap-3 text-sm text-base-content/80">
              <div class="flex items-center gap-3 justify-between">
                <StatusLabel status={item.statusInfo.status} text={item.statusInfo.text} />
                <Toggle
                  checked={item.checked}
                  disabled={!item.server.enabled || item.server.status !== 'ready'}
                  onChange={(value) => toggleSelection(item.server.id, value)}
                />
              </div>

              <div class="flex flex-wrap gap-2 text-xs text-base-content/70">
                <span class="px-2 py-0.5 rounded bg-base-200">工具 {item.server.tools.length}</span>
                <span class="px-2 py-0.5 rounded bg-base-200">
                  命令 {item.server.command}
                </span>
                {#if item.server.lastSyncAt}
                  <span class="px-2 py-0.5 rounded bg-base-200">
                    最近同步 {new Date(item.server.lastSyncAt).toLocaleString('zh-CN')}
                  </span>
                {/if}
              </div>

              {#if item.server.description}
                <p class="text-xs leading-relaxed text-base-content/70">
                  {item.server.description}
                </p>
              {/if}

              {#if item.server.lastError && item.server.status === 'error'}
                <div class="text-xs text-error bg-error/10 rounded-md px-3 py-2">
                  {item.server.lastError}
                </div>
              {/if}
            </div>
          </TableBaseRow>
      {/each}
    </TableGroup>

    <div class="flex gap-3 pt-4 justify-end">
      <Button
        variant="gray"
        size="sm"
        on:click={handleReset}
        disabled={!hasChanges()}
      >
        重置
      </Button>

      <Button
        size="sm"
        on:click={handleSave}
        disabled={!hasChanges() || saving}
      >
        {saving ? '保存中...' : '保存'}
      </Button>
    </div>
  {/if}
</div>
