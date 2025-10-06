<script lang="ts">
  import { onMount } from "svelte";
  import { Server, RefreshCw, ChevronsUpDown } from "@lucide/svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import DropDown from "$lib/components/ui/DropDown.svelte";
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import type { McpServer, McpServerConfig } from "$lib/types";

  let currentServers = $state<McpServerConfig[]>(
    chatState.currentChat?.mcpServers || [],
  );
  let originalServers = $state<McpServerConfig[]>(
    chatState.currentChat?.mcpServers || [],
  );
  let saving = $state(false);
  let refreshing = $state(false);
  let expandedTools = $state<Record<string, boolean>>({});

  const executionModeOptions = [
    { value: "auto", label: "自动执行" },
    { value: "manual", label: "手动执行" },
  ];

  onMount(() => {
    if (!mcpState.initialized) {
      mcpActions.loadServers().catch((error) => {
        console.error("Failed to load MCP servers:", error);
      });
    }
  });

  $effect(() => {
    currentServers = chatState.currentChat?.mcpServers || [];
    originalServers = chatState.currentChat?.mcpServers || [];
  });

  const hasChanges = $derived(() => {
    return JSON.stringify(currentServers) !== JSON.stringify(originalServers);
  });

  // Only show enabled servers with ready status and at least one enabled tool
  const availableServers = $derived(() =>
    mcpState.servers.filter(
      (server) =>
        server.enabled &&
        server.status === "ready" &&
        server.enabledTools.length > 0,
    ),
  );

  const decoratedServers = $derived(() => {
    return availableServers().map((server) => {
      const config = currentServers.find((s) => s.serverId === server.id);
      return {
        server,
        checked: !!config,
        executionMode: config?.executionMode || "auto",
      };
    });
  });

  async function toggleSelection(serverId: string, selected: boolean) {
    if (selected) {
      const exists = currentServers.find((s) => s.serverId === serverId);
      if (!exists) {
        // Find the server to get its enabled tools from settings
        const server = mcpState.servers.find((s) => s.id === serverId);
        const enabledTools = server?.enabledTools || [];

        currentServers = [
          ...currentServers,
          { serverId, executionMode: "auto", enabledTools },
        ];
      }
    } else {
      currentServers = currentServers.filter((s) => s.serverId !== serverId);
    }

    // Auto-save
    await saveChanges();
  }

  async function handleExecutionModeChange(
    serverId: string,
    mode: "auto" | "manual",
  ) {
    currentServers = currentServers.map((s) =>
      s.serverId === serverId ? { ...s, executionMode: mode } : s,
    );

    // Auto-save
    await saveChanges();
  }

  function toggleTools(serverId: string) {
    expandedTools[serverId] = !expandedTools[serverId];
  }

  async function saveChanges() {
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
      console.error("Failed to update MCP servers:", error);
      await chatActions.loadChats();
    } finally {
      saving = false;
    }
  }

  async function handleRefresh() {
    refreshing = true;
    try {
      await mcpActions.loadServers(true);
    } catch (error) {
      console.error("Failed to refresh MCP servers:", error);
    } finally {
      refreshing = false;
    }
  }
</script>

<div class="flex-1 mt-1 p-0 space-y-3">
  <div class="flex items-center justify-between">
    <div class="text-sm text-base-content/70">
      {#if !chatState.currentChat}
        请先选择或创建聊天
      {:else}
        已选中 {currentServers.length} 个服务器
      {/if}
    </div>

    <Button
      on:click={handleRefresh}
      variant="clear"
      size="sm"
      disabled={refreshing}
    >
      <RefreshCw class={refreshing ? "animate-spin" : ""} size={14} />
      刷新列表
    </Button>
  </div>

  {#if !chatState.currentChat}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">请先选择或创建聊天</p>
      <p class="text-sm">MCP 服务器配置将与聊天关联</p>
    </div>
  {:else if availableServers().length === 0}
    <div class="text-center py-8 text-base-content/70">
      <Server size={48} class="mx-auto mb-4 text-base-content/40" />
      <p class="mb-2">暂无可用的 MCP 服务器</p>
      <p class="text-sm">请在应用设置中配置 MCP 服务器</p>
    </div>
  {:else}
    <TableGroup>
      {#each decoratedServers() as item (item.server.id)}
        <TableBaseRow
          label={item.server.displayName ?? item.server.name}
          layout="vertical"
        >
          {#snippet rightContent()}
            <Toggle
              checked={item.checked}
              onChange={(value) => toggleSelection(item.server.id, value)}
            />
          {/snippet}

          <div class="flex flex-col gap-3 text-sm text-base-content/80">
            <!-- Execution mode and tools button -->
            <div class="flex items-center gap-2 justify-between">
              <div class="flex items-center gap-1">                
                <button
                  class="flex items-center gap-1 text-xs text-base-content/60 hover:text-base-content hover:bg-base-200 rounded py-0.5 transition-colors"
                  onclick={() => toggleTools(item.server.id)}
                >
                  <span>{item.server.enabledTools.length} enabled tools</span>
                  <ChevronsUpDown size={12} />
                </button>
              </div>
              {#if item.checked}
                <div>
                  <DropDown
                    options={executionModeOptions}
                    selectedValue={item.executionMode}
                    disabled={!item.checked}
                    onSelect={(value) =>
                      handleExecutionModeChange(
                        item.server.id,
                        value as "auto" | "manual",
                      )}
                    minWidth="min-w-28"
                    buttonClass="text-xs"
                  />
                </div>
              {/if}
            </div>

            <!-- Expanded tools list -->
            {#if expandedTools[item.server.id] && item.server.tools.length > 0}
              <div class="flex flex-wrap gap-1">
                {#each item.server.enabledTools as tool}
                  <span
                    class="px-2 py-0.5 text-xs rounded-full bg-primary/10 text-primary"
                  >
                    {tool}
                  </span>
                {/each}
              </div>
            {/if}

            <!-- Description -->
            {#if item.server.description}
              <p class="text-xs leading-relaxed text-base-content/70">
                {item.server.description}
              </p>
            {/if}
          </div>
        </TableBaseRow>
      {/each}
    </TableGroup>
  {/if}
</div>
