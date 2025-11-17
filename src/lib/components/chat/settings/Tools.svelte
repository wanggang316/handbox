<script lang="ts">
  import { onMount } from "svelte";
  import {
    Server,
    RefreshCw,
    ChevronDown,
    ChevronUp,
    Trash2,
    Settings2,
  } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import {
    mcpState,
    mcpActions,
    setupMcpServersUpdatedListener,
    cleanupMcpServersUpdatedListener,
  } from "$lib/states/mcp.svelte";
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
    // 注册跨窗口事件监听器（用于同步 MCP 服务器状态）
    setupMcpServersUpdatedListener().catch((error) => {
      console.error("Failed to setup MCP servers updated listener:", error);
    });

    if (!mcpState.initialized) {
      mcpActions.loadServers().catch((error) => {
        console.error("Failed to load MCP servers:", error);
      });
    }

    // 清理函数
    return () => {
      cleanupMcpServersUpdatedListener();
    };
  });

  $effect(() => {
    currentServers = chatState.currentChat?.mcpServers || [];
    originalServers = chatState.currentChat?.mcpServers || [];
  });

  // 监听 MCP 服务器更新事件，自动刷新
  $effect(() => {
    if (mcpState.needsRefresh) {
      console.log("[Tools] MCP servers needs refresh, reloading...");
      mcpActions.loadServers(true).catch((error: Error) => {
        console.error("[Tools] Failed to reload MCP servers:", error);
      });

      // 重新加载当前聊天配置（以防 MCP 被删除或解除关联）
      if (chatState.currentChat?.id) {
        console.log(
          "[Tools] Reloading current chat to sync MCP configuration..."
        );
        const chatId = chatState.currentChat.id;
        chatActions.switchToChat(chatId).catch((error: Error) => {
          console.error("[Tools] Failed to reload current chat:", error);
        });
      }
    }
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

  // 检测当前聊天中已配置但已关闭的 MCP 服务器
  const disabledConfiguredServers = $derived(() => {
    if (!currentServers.length) return [];

    return currentServers
      .map((config) => {
        const server = mcpState.servers.find((s) => s.id === config.serverId);
        // 服务器存在但未启用，或状态不是 ready
        if (server && (!server.enabled || server.status !== "ready")) {
          return {
            config,
            server,
            reason: !server.enabled ? "disabled" : "not_ready",
          };
        }
        // 服务器已被删除
        if (!server) {
          return {
            config,
            server: null,
            reason: "deleted" as const,
          };
        }
        return null;
      })
      .filter((item) => item !== null);
  });

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

  // 删除已关闭的 MCP 服务器配置
  async function removeDisabledServer(serverId: string) {
    currentServers = currentServers.filter((s) => s.serverId !== serverId);
    await saveChanges();
  }

  // 跳转到 MCP 服务器设置页面
  async function navigateToMcpSettings(serverId: string) {
    try {
      await openSettingsWindow(`/mcp/${serverId}`);
    } catch (error) {
      console.error("Failed to open settings window:", error);
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
    {:else if availableServers().length === 0 && disabledConfiguredServers().length === 0}
      <div class="text-center py-8 text-base-content/70">
        <p class="text-sm mb-2">暂无可用的 MCP 服务器</p>
        <p class="text-xs">请在应用设置中配置并开启 MCP 服务器</p>
      </div>
    {:else}
      <!-- 可用的 MCP 服务器 -->
      {#if availableServers().length > 0}
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
                      <Select
                        options={executionModeOptions}
                        selectedValue={item.executionMode}
                        disabled={!item.checked}
                        onSelect={(value) =>
                          handleExecutionModeChange(
                            item.server.id,
                            value as "auto" | "manual"
                          )}
                        size="sm"
                        autoWidth={true}
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

      <!-- 已关闭或不可用的 MCP 服务器 -->
      {#if disabledConfiguredServers().length > 0}
        <div class="mt-3">
          <div class="text-[12px] text-base-content/40 pl-2 mb-1">
            已关闭的服务器 ({disabledConfiguredServers().length})
          </div>
          <TableGroup>
            {#each disabledConfiguredServers() as item (item.config.serverId)}
              <TableBaseRow
                label={item.server?.displayName ??
                  item.server?.name ??
                  item.config.serverId}
                layout="vertical"
              >
                {#snippet rightContent()}
                  <div class="flex items-center gap-1">
                    <!-- 跳转到设置按钮 -->
                    {#if item.server}
                      <IconButton
                        icon={Settings2}
                        iconSize={16}
                        onclick={() =>
                          navigateToMcpSettings(item.config.serverId)}
                      />
                    {/if}
                    <!-- 删除按钮 -->
                    <IconButton
                      icon={Trash2}
                      iconSize={16}
                      onclick={() => removeDisabledServer(item.config.serverId)}
                    />
                  </div>
                {/snippet}

                <div class="flex flex-col gap-1 text-sm opacity-60">
                  <div class="text-xs text-base-content/60">
                    {#if item.reason === "disabled"}
                      <span class="text-warning/80">● 服务器已关闭</span>
                    {:else if item.reason === "not_ready"}
                      <span class="text-error/80">● 服务器未就绪</span>
                    {:else if item.reason === "deleted"}
                      <span class="text-error/80">● 服务器已删除</span>
                    {/if}
                  </div>
                  <div class="text-xs text-base-content/50">
                    {#if item.reason === "disabled"}
                      此服务器已在全局设置中关闭，请启用后再使用
                    {:else if item.reason === "not_ready"}
                      此服务器状态异常，请检查配置
                    {:else if item.reason === "deleted"}
                      此服务器已被删除，建议移除此配置
                    {/if}
                  </div>
                </div>
              </TableBaseRow>
            {/each}
          </TableGroup>
        </div>
      {/if}
    {/if}
  {/if}
</div>
