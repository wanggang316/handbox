<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import TableGroup from '$lib/components/ui/table/TableGroup.svelte';
  import StatusLabel from '$lib/components/ui/StatusLabel.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import McpServerFormModal from '$lib/components/settings/McpServerFormModal.svelte';
  import { mcpState, mcpActions } from '$lib/states/mcp.svelte';
  import type { McpServer, McpServerStatus } from '$lib/types';
  import { LoaderCircle, Puzzle, ChevronRight } from '@lucide/svelte';

  let showFormModal = $state(false);
  let editingServer = $state<McpServer | null>(null);

  onMount(() => {
    if (!mcpState.initialized) {
      mcpActions.loadServers().catch(error => {
        console.error('Failed to load MCP servers:', error);
      });
    }
  });

  function handleServerClick(server: McpServer) {
    goto(`/settings/mcp/${server.id}`);
  }

  function handleAddServer() {
    editingServer = null;
    showFormModal = true;
  }

  function closeModal() {
    showFormModal = false;
    editingServer = null;
  }

  function getServerStatus(server: McpServer): 'enabled' | 'disabled' | 'idle' | 'error' {
    if (!server.enabled) return 'disabled';

    switch (server.status) {
      case 'ready':
        return 'enabled';
      case 'error':
        return 'error';
      case 'inactive':
        return 'idle';
      default:
        return 'idle';
    }
  }

  function getServerStatusText(server: McpServer): string {
    if (!server.enabled) return '已禁用';

    switch (server.status) {
      case 'ready':
        return '就绪';
      case 'error':
        return '错误';
      case 'inactive':
        return '未激活';
      default:
        return '未知';
    }
  }

  function getConnectionTypeLabel(connectionType: string): string {
    switch (connectionType) {
      case 'stdio':
        return 'stdio';
      case 'sse':
        return 'SSE';
      case 'http':
        return 'HTTP';
      default:
        return connectionType;
    }
  }
</script>

<div class="p-6 pr-8 pt-14 flex flex-col gap-y-4">
  <!-- 加载状态 -->
  {#if mcpState.isLoading}
    <div class="flex items-center justify-center py-8">
      <LoaderCircle class="h-6 w-6 animate-spin text-base-content/60" />
      <span class="ml-2 text-sm text-base-content/70">正在加载 MCP 服务器...</span>
    </div>
  {/if}

  <div class="rounded-[20px] overflow-hidden">
    <!-- MCP 服务器列表 -->
    <TableGroup>
      {#each mcpState.servers as server (server.id)}
        <button
          class="w-full hover:bg-base-300 group px-6 py-4 border-b border-base-300 last:border-b-0"
          onclick={() => handleServerClick(server)}
        >
          <div class="flex items-center justify-between">
            <div class="flex-1 text-left">
              <div class="flex items-center gap-2 mb-1">
                <span class="text-sm font-medium text-base-content">{server.displayName || server.name}</span>
                <span class="px-2 py-0.5 text-xs rounded-full bg-primary/10 text-primary">
                  {getConnectionTypeLabel(server.connectionType)}
                </span>
              </div>
              {#if server.description}
                <p class="text-xs text-base-content/60">{server.description}</p>
              {/if}
            </div>

            <div class="flex items-center gap-3">
              <StatusLabel status={getServerStatus(server)} text={getServerStatusText(server)} />
              <ChevronRight size={16} class="text-base-content/50 group-hover:text-base-content transition-colors" />
            </div>
          </div>
        </button>
      {/each}

      <!-- 空状态 -->
      {#if !mcpState.isLoading && mcpState.servers.length === 0}
        <div class="p-8 text-center">
          <Puzzle class="h-12 w-12 text-base-content/50 mx-auto mb-4" />
          <p class="text-base text-base-content/70 mb-4">
            添加 MCP 服务器来扩展 AI 能力
          </p>
          <Button variant="primary" size="sm" on:click={handleAddServer}>
            添加 MCP 服务器
          </Button>
        </div>
      {/if}
    </TableGroup>
  </div>

  <!-- 添加按钮 -->
  {#if mcpState.servers.length > 0}
    <div>
      <Button variant="gray" size="sm" on:click={handleAddServer}>
        添加其它 MCP 服务器
      </Button>
    </div>
  {/if}
</div>

<!-- 添加/编辑弹窗 -->
<McpServerFormModal
  open={showFormModal}
  server={editingServer}
  onClose={closeModal}
/>
