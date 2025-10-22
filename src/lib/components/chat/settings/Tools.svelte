<script lang="ts">
  import { onMount } from "svelte";
  import { Server, RefreshCw, ChevronDown, ChevronUp } from "@lucide/svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import DropDown from "$lib/components/ui/DropDown.svelte";
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import type { McpServer, McpServerConfig } from "$lib/types";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import ArrowButton from "$lib/components/ui/ArrowButton.svelte";

  let currentServers = $state<McpServerConfig[]>(
    chatState.currentChat?.mcpServers || []
  );
  let originalServers = $state<McpServerConfig[]>(
    chatState.currentChat?.mcpServers || []
  );
  let saving = $state(false);
  let refreshing = $state(false);
  let expandedTools = $state<Record<string, boolean>>({});

  let isCollapsed = $state(false);
  let isHovering = $state(false);

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

  function toggleCollapse() {
    isCollapsed = !isCollapsed;
  }

  // Only show enabled servers with ready status and at least one enabled tool
  const availableServers = $derived(() =>
    mcpState.servers.filter(
      (server) =>
        server.enabled &&
        server.status === "ready" &&
        server.enabledTools.length > 0
    )
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
    mode: "auto" | "manual"
  ) {
    currentServers = currentServers.map((s) =>
      s.serverId === serverId ? { ...s, executionMode: mode } : s
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

<div class="flex-1 mt-1">
  <button
    type="button"
    class="flex w-full items-center justify-between my-1 px-2 text-xs cursor-pointer text-base-content/80 hover:text-base-content"
    onclick={toggleCollapse}
    onmouseenter={() => (isHovering = true)}
    onmouseleave={() => (isHovering = false)}
  >
    <span>工具</span>
    {#if isHovering}
      {#if isCollapsed}
        <ChevronDown size={16} />
      {:else}
        <ChevronUp size={16} />
      {/if}
    {/if}
  </button>

  {#if !isCollapsed}
    <div class="flex items-center justify-between pl-2 pr-1">
      <div class="text-[12px] text-base-content/50">
        {#if !chatState.currentChat}
          请先选择或创建聊天
        {:else}
          已选中 {currentServers.length} 个服务器
        {/if}
      </div>

      <IconButton
        icon={RefreshCw}
        iconSize={14}
        onclick={handleRefresh}
        disabled={refreshing}
        customClass={refreshing ? "animate-spin" : ""}
      />
    </div>

    {#if !chatState.currentChat}
      <div class="text-center py-8 text-base-content/70">
        <p class="text-sm mb-2">请先选择或创建聊天</p>
        <p class="text-xs">MCP 服务器配置将与聊天关联</p>
      </div>
    {:else if availableServers().length === 0}
      <div class="text-center py-8 text-base-content/70">
        <p class="text-sm mb-2">暂无可用的 MCP 服务器</p>
        <p class="text-xs">请在应用设置中配置并开启 MCP 服务器</p>
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

            <div class="flex flex-col gap-1 text-sm text-base-content/80">
              <!-- Execution mode and tools button -->
              <div class="flex items-center gap-2 justify-between">
                <div class="flex items-center gap-1 pt-1">
                  <ArrowButton
                    label="{item.server.enabledTools.length} enabled tools"
                    onclick={() => toggleTools(item.server.id)}
                  />
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
                          value as "auto" | "manual"
                        )}
                      minWidth="min-w-28"
                      buttonTextSize="text-[12px]"
                      buttonIconSize="12"
                      buttonPx="1"
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
            </div>
          </TableBaseRow>
        {/each}
      </TableGroup>
    {/if}
  {/if}
</div>
