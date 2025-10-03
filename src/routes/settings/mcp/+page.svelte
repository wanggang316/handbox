<script lang="ts">
  import { onMount } from 'svelte';
  import TableGroup from '$lib/components/ui/table/TableGroup.svelte';
  import TableBaseRow from '$lib/components/ui/table/TableBaseRow.svelte';
  import Toggle from '$lib/components/ui/Toggle.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import StatusLabel from '$lib/components/ui/StatusLabel.svelte';
  import McpServerFormModal from '$lib/components/settings/McpServerFormModal.svelte';
  import { mcpState, mcpActions } from '$lib/states/mcp.svelte';
  import type { McpServer, McpServerStatus, CreateMcpServerRequest, UpdateMcpServerRequest } from '$lib/types';
  import {
    Plus,
    RefreshCw,
    Pencil,
    Trash2,
    ChevronsUpDown
  } from '@lucide/svelte';

  let expandedStates = $state<Record<string, boolean>>({});
  let showFormModal = $state(false);
  let editingServer = $state<McpServer | null>(null);
  let pendingMap = $state<Record<string, boolean>>({});
  let refreshingMap = $state<Record<string, boolean>>({});
  let deletingMap = $state<Record<string, boolean>>({});

  const decoratedServers = $derived(() =>
    mcpState.servers.map(server => ({
      server,
      statusInfo: mapStatus(server.status as McpServerStatus)
    }))
  );

  onMount(() => {
    if (!mcpState.initialized) {
      mcpActions.loadServers().catch(error => {
        console.error('Failed to load MCP servers:', error);
      });
    }
  });

  function toggleTools(id: string) {
    expandedStates[id] = !expandedStates[id];
  }

  function openCreateModal() {
    editingServer = null;
    showFormModal = true;
  }

  function openEditModal(server: McpServer) {
    console.log('openEditModal', server);
    editingServer = server;
    // expandedStates = { ...expandedStates, [server.id]: true };
    showFormModal = true;
  }

  function closeModal() {
    showFormModal = false;
  }

  function setPending(id: string, value: boolean) {
    pendingMap = { ...pendingMap, [id]: value };
  }

  function setRefreshing(id: string, value: boolean) {
    refreshingMap = { ...refreshingMap, [id]: value };
  }

  function setDeleting(id: string, value: boolean) {
    deletingMap = { ...deletingMap, [id]: value };
  }

  async function handleToggle(server: McpServer, enabled: boolean) {
    setPending(server.id, true);
    try {
      await mcpActions.toggleServer({ serverId: server.id, enabled });
    } catch (error) {
      console.error('Failed to toggle MCP server:', error);
    } finally {
      setPending(server.id, false);
    }
  }

  async function handleRefresh(server: McpServer) {
    setRefreshing(server.id, true);
    try {
      await mcpActions.refreshServer({ serverId: server.id });
    } catch (error) {
      console.error('Failed to refresh MCP server:', error);
    } finally {
      setRefreshing(server.id, false);
    }
  }

  async function handleDelete(server: McpServer) {
    if (!confirm(`确定要删除 ${server.displayName ?? server.name} 吗？`)) {
      return;
    }
    setDeleting(server.id, true);
    try {
      await mcpActions.deleteServer(server.id);
    } catch (error) {
      console.error('Failed to delete MCP server:', error);
    } finally {
      setDeleting(server.id, false);
    }
  }

  async function handleSave(payload: { mode: 'create' | 'update'; data: CreateMcpServerRequest | UpdateMcpServerRequest }) {
    const { mode, data } = payload;
    try {
      if (mode === 'create') {
        await mcpActions.createServer(data as CreateMcpServerRequest);
      } else if (editingServer) {
        await mcpActions.updateServer(editingServer.id, data as UpdateMcpServerRequest);
      }
    } catch (error) {
      console.error('Failed to save MCP server:', error);
    }
  }

  function mapStatus(status: McpServerStatus): { status: 'enabled' | 'disabled' | 'idle' | 'error'; text: string } {
    switch (status) {
      case 'ready':
        return { status: 'enabled', text: '已就绪' };
      case 'inactive':
        return { status: 'idle', text: '未启用' };
      case 'error':
        return { status: 'error', text: '异常' };
      default:
        return { status: 'disabled', text: '未知' };
    }
  }

  function formatLastSync(timestamp?: number): string {
    if (!timestamp) return '尚未同步';
    try {
      return new Date(timestamp).toLocaleString('zh-CN');
    } catch (error) {
      console.error('Failed to format timestamp:', error);
      return '未知';
    }
  }

</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <div class="flex items-center justify-between">
    <div class="text-sm text-base-content/70">
      {#if mcpState.isLoading}
        正在加载 MCP 服务器...
      {:else}
        共 {mcpState.servers.length} 个服务器
      {/if}
    </div>
    
  </div>

  {#if decoratedServers().length > 0}
    <TableGroup>
      {#each decoratedServers() as item (item.server.id)}
        {#snippet controls()}
            <div class="flex items-center gap-3">
              <StatusLabel status={item.statusInfo.status} text={item.statusInfo.text} />
              <Toggle
                checked={item.server.enabled}
                disabled={pendingMap[item.server.id] || deletingMap[item.server.id]}
                onChange={(value) => handleToggle(item.server, value)}
              />
              <button
                class="p-1.5 rounded hover:bg-base-300 transition-colors"
                title="刷新工具列表"
                disabled={refreshingMap[item.server.id] || pendingMap[item.server.id]}
                onclick={() => handleRefresh(item.server)}
              >
                <RefreshCw
                  size={14}
                  class={refreshingMap[item.server.id] ? 'animate-spin text-primary' : 'text-base-content/80'}
                />
              </button>
              <button
                class="p-1.5 rounded hover:bg-base-300 transition-colors"
                title="编辑"
                onclick={() => openEditModal(item.server)}
              >
                <Pencil size={14} class="text-base-content/80" />
              </button>
              <button
                class="p-1.5 rounded hover:bg-error/10 transition-colors"
                title="删除"
                disabled={deletingMap[item.server.id]}
                onclick={() => handleDelete(item.server)}
              >
                <Trash2 size={14} class="text-error" />
              </button>
            </div>
          {/snippet}

          <TableBaseRow
            label={item.server.displayName ?? item.server.name}
            layout="vertical"
            rightContent={controls}
          >
            <div class="space-y-3 text-sm text-base-content/80">
              <div class="flex flex-wrap gap-2 text-xs">
                <span class="px-2 py-0.5 rounded-md bg-primary/10 text-primary font-medium">
                  {item.server.connectionType === 'stdio' ? '进程连接' :
                   item.server.connectionType === 'sse' ? 'SSE连接' :
                   item.server.connectionType === 'http' ? 'HTTP连接' : '未知连接'}
                </span>

                {#if item.server.connectionType === 'stdio'}
                  <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                    命令: <span class="font-mono text-base-content">{item.server.command}</span>
                  </span>
                  {#if item.server.args.length}
                    <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                      参数: {item.server.args.join(', ')}
                    </span>
                  {/if}
                  {#if item.server.workingDir}
                    <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                      工作目录: {item.server.workingDir}
                    </span>
                  {/if}
                {:else}
                  {#if item.server.endpoint}
                    <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                      端点: <span class="font-mono text-base-content">{item.server.endpoint}</span>
                    </span>
                  {/if}
                  {#if item.server.timeoutMs}
                    <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                      超时: {item.server.timeoutMs}ms
                    </span>
                  {/if}
                {/if}

                <span class="px-2 py-0.5 rounded-md bg-base-200 text-base-content/70">
                  最近同步: {formatLastSync(item.server.lastSyncAt)}
                </span>
              </div>

              <div class="space-y-2">
                <button
                  class="flex items-center gap-1 text-xs text-primary hover:text-primary/80"
                  type="button"
                  onclick={() => toggleTools(item.server.id)}
                >
                  <span>工具列表</span>
                  <ChevronsUpDown size={12} />
                </button>

                {#if expandedStates[item.server.id]}
                  <div class="flex flex-wrap gap-2">
                    {#if item.server.tools.length === 0}
                      <span class="text-xs text-base-content/60">尚未同步到任何工具</span>
                    {:else}
                      {#each item.server.tools as tool (tool.name)}
                        <span class="px-2 py-0.5 rounded bg-base-300/60 text-xs text-base-content">
                          {tool.name}
                        </span>
                      {/each}
                    {/if}
                  </div>
                {/if}
              </div>

              {#if item.server.description}
                <p class="text-xs text-base-content/70 leading-relaxed">
                  {item.server.description}
                </p>
              {/if}

              {#if item.server.lastError}
                <div class="text-xs text-error bg-error/10 rounded-md px-3 py-2">
                  {item.server.lastError}
                </div>
              {/if}
            </div>
          </TableBaseRow>
      {/each}
    </TableGroup>
    <div class="flex justify-start">
      <Button size="sm" variant="primary" on:click={openCreateModal}>
        <Plus size={14} />
        新增服务器
      </Button>
    </div>
  {:else if mcpState.isLoading}
    <div class="flex items-center justify-center py-12 text-base-content/70 text-sm">
      正在加载 MCP 服务器...
    </div>
  {:else}
    <div class="flex flex-col items-center gap-3 py-12 text-base-content/70">
      <p class="text-base">暂无配置的 MCP 服务器</p>
      <p class="text-sm">点击“新增服务器”开始配置</p>
    </div>
  {/if}
</div>

<McpServerFormModal
  bind:open={showFormModal}
  bind:server={editingServer}
  onClose={closeModal}
  onSave={handleSave}
/>
