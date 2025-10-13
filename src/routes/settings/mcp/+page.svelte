<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import McpServerFormModal from "$lib/components/settings/McpServerFormModal.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import type { McpServer, McpServerStatus } from "$lib/types";
  import { formatDateTime } from "$lib/utils/date";
  import {
    LoaderCircle,
    Puzzle,
    ChevronsUpDown,
    Settings2,
  } from "@lucide/svelte";

  let showFormModal = $state(false);
  let editingServer = $state<McpServer | null>(null);
  let expandedTools = $state<Record<string, boolean>>({});

  onMount(() => {
    if (!mcpState.initialized) {
      mcpActions.loadServers().catch((error) => {
        console.error("Failed to load MCP servers:", error);
      });
    }
  });

  function handleAddServer() {
    editingServer = null;
    showFormModal = true;
  }

  function closeModal() {
    showFormModal = false;
    editingServer = null;
  }

  function getConnectionTypeLabel(connectionType: string): string {
    switch (connectionType) {
      case "stdio":
        return "stdio";
      case "sse":
        return "SSE";
      case "http":
        return "HTTP";
      default:
        return connectionType;
    }
  }

  function toggleTools(serverId: string) {
    expandedTools[serverId] = !expandedTools[serverId];
  }

  async function handleToggleServer(server: McpServer, enabled: boolean) {
    try {
      await mcpActions.toggleServer({ serverId: server.id, enabled });
    } catch (error) {
      console.error("Failed to toggle MCP server:", error);
    }
  }

  function handleEditServer(server: McpServer, event: MouseEvent) {
    goto(`/settings/mcp/${server.id}`);
  }
</script>

<div class="p-6 pr-8 pt-14 flex flex-col gap-y-4">
  <!-- 加载状态 -->
  {#if mcpState.isLoading}
    <div class="flex items-center justify-center py-8">
      <LoaderCircle class="h-6 w-6 animate-spin text-base-content/60" />
      <span class="ml-2 text-sm text-base-content/70"
        >正在加载 MCP 服务器...</span
      >
    </div>
  {/if}

  <div class="rounded-[20px] overflow-hidden">
    <!-- MCP 服务器列表 -->
    <TableGroup>
      {#each mcpState.servers as server (server.id)}
        <div class="w-full px-6 py-4">
          <div class="flex items-center justify-between mb-1">
            <div class="flex flex-1 items-center gap-2">
              <span class="text-sm font-medium text-base-content"
                >{server.displayName || server.name}</span
              >
              <span
                class="px-2 py-0.5 text-xs rounded-full bg-primary/10 text-primary"
              >
                {getConnectionTypeLabel(server.connectionType)}
              </span>
            </div>

            <div class="flex items-center gap-2">
              <Toggle
                checked={server.enabled}
                onChange={(enabled) => handleToggleServer(server, enabled)}
              />
              <IconButton
                icon={Settings2}
                iconSize={16}
                ariaLabel="编辑"
                size="w-7 h-7"
                onclick={(e) => handleEditServer(server, e)}
              />
            </div>
          </div>
          <div>
            <!-- 工具统计信息或错误信息 -->
            {#if server.status === "error" && server.lastError}
              <div class="text-xs text-error">
                {server.lastError.message}
              </div>
            {:else if server.tools.length > 0}
              <div class="flex items-center gap-2">
                <button
                  class="flex items-center gap-1 text-xs text-base-content/60 hover:text-base-content hover:bg-base-200 rounded px-1 -ml-1 py-0.5 transition-colors"
                  onclick={() => toggleTools(server.id)}
                >
                  <span
                    >{server.tools.length} tools, {server.enabledTools.length} enabled</span
                  >
                  <ChevronsUpDown size={12} />
                </button>
                {#if server.lastSyncAt}
                  <span class="text-xs text-base-content/50">
                    · {formatDateTime(server.lastSyncAt)}
                  </span>
                {/if}
              </div>
            {:else}
              <div class="flex items-center gap-2">
                <div class="text-xs text-base-content/60">
                  0 tools, 0 enabled
                </div>
                {#if server.lastSyncAt}
                  <span class="text-xs text-base-content/50">
                    · {formatDateTime(server.lastSyncAt)}
                  </span>
                {/if}
              </div>
            {/if}
            <!-- 工具列表 -->
            {#if expandedTools[server.id] && server.tools.length > 0}
              <div class="flex flex-wrap gap-1 mt-2">
                {#each server.tools as tool}
                  <span
                    class="px-2 py-0.5 text-xs rounded-full {server.enabledTools.includes(
                      tool.name
                    )
                      ? 'bg-primary/10 text-primary'
                      : 'bg-base-300 text-base-content/60'}"
                  >
                    {tool.name}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        </div>
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
        添加 MCP 服务器
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
