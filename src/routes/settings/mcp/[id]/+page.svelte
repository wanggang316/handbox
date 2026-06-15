<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import McpServerFormModal from "$lib/components/settings/McpServerFormModal.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import { updateToolEnabled } from "$lib/api";
  import { countChatsUsingServer, removeMcpServerFromChats } from "$lib/api/mcp";
  import type { McpServer } from "$lib/types";
  import { formatDateTime } from "$lib/utils/date";
  import {
    ChevronLeft,
    RefreshCw,
    SquarePen,
    Trash2,
    ChevronDown,
    ChevronRight,
  } from "@lucide/svelte";

  let serverId = $state("");
  let activeTab = $state("tools");
  let isRefreshing = $state(false);
  let showDeleteConfirm = $state(false);
  let showDisableConfirm = $state(false);
  let showEditModal = $state(false);
  let confirmModalRef: any;
  let relatedChatsCount = $state(0);

  // 记录每个项目的展开状态
  let expandedTools = $state<Record<string, boolean>>({});
  let expandedPrompts = $state<Record<string, boolean>>({});
  let expandedResources = $state<Record<string, boolean>>({});

  // 获取当前服务器
  const server = $derived<McpServer | undefined>(
    mcpState.servers.find((s) => s.id === serverId)
  );

  // 表单数据
  let formData = $state({
    enabled: false,
  });

  onMount(() => {
    serverId = $page.params.id || "";
    loadServer();
  });

  async function loadServer() {
    if (!serverId) return;

    try {
      if (!mcpState.initialized) {
        await mcpActions.loadServers();
      }

      if (server) {
        formData = {
          enabled: server.enabled,
        };
      } else {
        console.error("MCP server not found:", serverId);
        goto("/settings/mcp");
      }
    } catch (error) {
      console.error("Failed to load MCP server:", error);
    }
  }

  // 监听服务器变化，更新表单数据
  $effect(() => {
    if (server) {
      formData = {
        enabled: server.enabled,
      };
    }
  });

  async function handleToggleBefore(enabled: boolean, previous: boolean) {
    if (!server) return true;

    if (!enabled && previous && server.enabled) {
      try {
        const count = await countChatsUsingServer(server.id);
        relatedChatsCount = count;
        if (count > 0) {
          showDisableConfirm = true;
          return false;
        }
      } catch (error) {
        console.error("Failed to count related chats:", error);
        // 如果检查失败，允许继续执行禁用操作
        return true;
      }
    }

    return true;
  }

  async function handleToggle(enabled: boolean) {
    if (!server) return;
    await performToggle(enabled);
  }

  async function performToggle(enabled: boolean) {
    if (!server) return;

    // 乐观更新UI
    const previousState = formData.enabled;
    formData.enabled = enabled;

    try {
      console.log("performToggle", server.id, enabled);
      await mcpActions.toggleServer({ serverId: server.id, enabled });
      console.log(
        `MCP server ${enabled ? "enabled" : "disabled"} successfully`
      );
    } catch (error) {
      console.error("Failed to toggle MCP server:", error);
      // 发生错误时回滚UI状态
      formData.enabled = previousState;
    }
  }

  async function handleDisableWithoutRemove() {
    await performToggle(false);
    showDisableConfirm = false;
  }

  async function handleDisableAndRemove() {
    if (!server) return;

    try {
      // 先移除会话中的 MCP 配置
      await removeMcpServerFromChats(server.id);
      // 再关闭 MCP 服务器
      await performToggle(false);
      showDisableConfirm = false;
    } catch (error) {
      console.error("Failed to disable and remove MCP server:", error);
    }
  }

  function handleCancelDisable() {
    if (server) {
      formData.enabled = server.enabled;
    }
    showDisableConfirm = false;
  }

  async function handleRefresh() {
    if (!server || isRefreshing) return;

    isRefreshing = true;
    try {
      await mcpActions.refreshServer({ serverId: server.id });
    } catch (error) {
      console.error("Failed to refresh MCP server:", error);
    } finally {
      isRefreshing = false;
    }
  }

  function handleEdit(event: MouseEvent) {
    console.log("Edit button clicked", event);
    if (!server) return;
    showEditModal = true;
  }

  function closeEditModal() {
    showEditModal = false;
  }

  async function handleSaveServer(data: {
    mode: "create" | "update";
    data: any;
  }) {
    if (data.mode === "update" && server) {
      await mcpActions.updateServer(server.id, data.data);
      console.log("MCP server updated successfully");
      // 刷新服务器数据
      await mcpActions.loadServers(true);
    } else if (data.mode === "create") {
      await mcpActions.createServer(data.data);
      console.log("MCP server created successfully");
    }
  }

  function handleDelete(event: MouseEvent) {
    console.log("Delete button clicked", event);
    if (!server) return;
    showDeleteConfirm = true;
  }

  async function confirmDelete() {
    if (!server) return;

    try {
      await mcpActions.deleteServer(server.id);
      console.log("MCP server deleted successfully");
      goto("/settings/mcp");
    } catch (error) {
      console.error("Failed to delete MCP server:", error);
      // 删除失败时触发关闭动画
      confirmModalRef?.modalRef?.handleClose();
    }
  }

  function handleBack() {
    goto("/settings/mcp");
  }

  const connectionTypeLabel = $derived(() => {
    if (!server) return "";
    switch (server.connectionType) {
      case "stdio":
        return "stdio";
      case "sse":
        return "SSE";
      case "http":
        return "HTTP";
      default:
        return server.connectionType;
    }
  });

  function toggleTool(toolName: string) {
    expandedTools[toolName] = !expandedTools[toolName];
  }

  function togglePrompt(promptName: string) {
    expandedPrompts[promptName] = !expandedPrompts[promptName];
  }

  function toggleResource(resourceUri: string) {
    expandedResources[resourceUri] = !expandedResources[resourceUri];
  }

  async function handleToolToggle(toolName: string, enabled: boolean) {
    if (!server) return;

    try {
      const updatedServer = await updateToolEnabled({
        serverId: server.id,
        toolName,
        enabled,
      });

      // 强制刷新服务器列表，确保列表页和详情页数据同步
      await mcpActions.loadServers(true);

      // 通知其他窗口 MCP 工具已更新
      mcpActions.notifyMcpServersUpdated("mcp-tool-toggled", {
        serverId: server.id,
        toolName,
        enabled,
      });
    } catch (error) {
      console.error("Failed to update tool enabled status:", error);
    }
  }
</script>

<!-- 页面布局：与 provider 详情页面一致 -->
<div class="flex flex-col h-screen">
  <!-- 粘性导航栏 -->
  <header class="text-base-content py-2 px-4 flex-shrink-0">
    <CircleButton
      icon={ChevronLeft}
      iconSize={22}
      ariaLabel="返回"
      size="w-8 h-8"
      variant="secondary"
      customClass="hover:text-base-content/80 z-10004 relative"
      onclick={handleBack}
    />
  </header>

  <!-- 主要内容区域 -->
  <main class="flex-grow overflow-y-auto p-6 pr-8">
    {#if server}
      <!-- 基本信息卡片 -->
      <TableGroup>
        <div class="px-6 py-4">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="text-sm text-base-content"
                >{server.displayName || server.name}</span
              >
              <span
                class="px-2 py-0.5 text-xs rounded-full bg-primary/10 text-primary"
              >
                {connectionTypeLabel()}
              </span>
            </div>
            <div class="flex flex-row items-center gap-4">
              <IconButton icon={SquarePen} onclick={handleEdit} />
              <IconButton icon={Trash2} onclick={handleDelete} />
              <IconButton
                icon={RefreshCw}
                onclick={handleRefresh}
                disabled={!server.enabled || isRefreshing}
                customClass={isRefreshing ? "animate-spin" : ""}
              />
              <Toggle
                checked={formData.enabled}
                onChangeBefore={handleToggleBefore}
                onChange={handleToggle}
              />
            </div>
          </div>
        </div>
      </TableGroup>

      <!-- 同步时间信息 -->
      {#if server.lastSyncAt}
        <div class="px-6 mt-2 mb-4 flex justify-end">
          <span class="text-xs text-base-content/60">
            最后同步: {formatDateTime(server.lastSyncAt)}
          </span>
        </div>
      {/if}

      <!-- 错误信息展示 -->
      {#if server.status === "error" && server.lastError}
        <div class="mt-4 p-4 rounded-lg bg-error/10 border border-error/20">
          <div
            class="text-sm text-error font-medium break-words whitespace-pre-wrap"
          >
            {server.lastError.message}
          </div>
        </div>
      {/if}

      <!-- Tab 导航（仅在非错误状态时显示） -->
      {#if server.status !== "error"}
        <Tabs
          value={activeTab}
          items={[
            { value: "tools", label: "工具" },
            { value: "prompts", label: "提示" },
            { value: "resources", label: "资源" },
          ]}
          onChange={(val) => {
            activeTab = val;
          }}
        />

        <!-- Tab 内容 -->
        {#if activeTab === "tools"}
          {#if server.tools.length === 0}
            <div class="text-center text-sm py-8 text-base-content/70">
              暂无工具数据
            </div>
          {:else}
            <div class="space-y-2 mt-4">
              {#each server.tools as tool}
                <TableGroup>
                  <TableBaseRow label={tool.name} layout="vertical">
                    <div class="flex items-start justify-between mb-3">
                      <div class="flex-1">
                        {#if tool.description}
                          <p class="text-xs text-base-content/70">
                            {tool.description}
                          </p>
                        {/if}
                      </div>
                      <Toggle
                        checked={server.enabledTools.includes(tool.name)}
                        onChange={(enabled) =>
                          handleToolToggle(tool.name, enabled)}
                      />
                      <!-- 工具开关不受服务器启用状态影响，可以随时配置 -->
                    </div>

                    {#if tool.inputSchema && typeof tool.inputSchema === "object" && "properties" in tool.inputSchema && Object.keys(tool.inputSchema.properties || {}).length > 0}
                      <button
                        class="flex items-center gap-1 text-xs text-base-content/80 hover:text-base-content/80 mt-2"
                        onclick={() => toggleTool(tool.name)}
                      >
                        {#if expandedTools[tool.name]}
                          <ChevronDown size={12} />
                          <span>参数</span>
                        {:else}
                          <ChevronRight size={12} />
                          <span
                            >参数 ({Object.keys(
                              tool.inputSchema.properties || {}
                            ).length})</span
                          >
                        {/if}
                      </button>

                      {#if expandedTools[tool.name]}
                        <div class="mt-3 pl-4 border-l-2 border-base-300">
                          <div class="space-y-2">
                            {#each Object.entries(tool.inputSchema.properties || {}) as [key, value]}
                              {@const propValue = value as {
                                type?: string;
                                description?: string;
                              }}
                              {@const requiredFields =
                                (tool.inputSchema as { required?: string[] })
                                  ?.required || []}
                              {@const isRequired = requiredFields.includes(key)}
                              <div class="text-xs">
                                <span class="font-mono text-primary">{key}</span
                                >
                                {#if isRequired}
                                  <span class="text-error ml-1">*</span>
                                {/if}
                                {#if propValue.type}
                                  <span class="text-base-content/60 ml-1"
                                    >({propValue.type})</span
                                  >
                                {/if}
                                {#if propValue.description}
                                  <span class="text-base-content/70 ml-2"
                                    >- {propValue.description}</span
                                  >
                                {/if}
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    {/if}
                  </TableBaseRow>
                </TableGroup>
              {/each}
            </div>
          {/if}
        {:else if activeTab === "prompts"}
          {#if server.prompts.length === 0}
            <div class="text-center text-sm py-8 text-base-content/70">
              暂无提示数据
            </div>
          {:else}
            <div class="space-y-4 mt-4">
              {#each server.prompts as prompt}
                <TableGroup>
                  <TableBaseRow label={prompt.name} layout="vertical">
                    {#if prompt.description}
                      <p class="text-xs text-base-content/70 mb-3">
                        {prompt.description}
                      </p>
                    {/if}

                    {#if prompt.arguments.length > 0}
                      <button
                        class="flex items-center gap-1 text-xs text-primary hover:text-primary/80 mt-2"
                        onclick={() => togglePrompt(prompt.name)}
                      >
                        {#if expandedPrompts[prompt.name]}
                          <ChevronDown size={14} />
                          <span>参数</span>
                        {:else}
                          <ChevronRight size={14} />
                          <span>参数 ({prompt.arguments.length})</span>
                        {/if}
                      </button>

                      {#if expandedPrompts[prompt.name]}
                        <div class="mt-3 pl-4 border-l-2 border-base-300">
                          <div class="space-y-2">
                            {#each prompt.arguments as arg}
                              <div class="text-xs">
                                <span class="font-mono text-primary"
                                  >{arg.name}</span
                                >
                                {#if arg.required}
                                  <span class="text-error ml-1">*</span>
                                {/if}
                                {#if arg.description}
                                  <span class="text-base-content/70 ml-2"
                                    >- {arg.description}</span
                                  >
                                {/if}
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    {/if}
                  </TableBaseRow>
                </TableGroup>
              {/each}
            </div>
          {/if}
        {:else if activeTab === "resources"}
          {#if server.resources.length === 0}
            <div class="text-center text-sm py-8 text-base-content/70">
              暂无资源数据
            </div>
          {:else}
            <div class="space-y-4 mt-4">
              {#each server.resources as resource}
                <TableGroup>
                  <TableBaseRow label={resource.name} layout="vertical">
                    {#if resource.description}
                      <p class="text-xs text-base-content/70 mb-3">
                        {resource.description}
                      </p>
                    {/if}

                    <button
                      class="flex items-center gap-1 text-xs text-primary hover:text-primary/80 mt-2"
                      onclick={() => toggleResource(resource.uri)}
                    >
                      {#if expandedResources[resource.uri]}
                        <ChevronDown size={14} />
                        <span>详情</span>
                      {:else}
                        <ChevronRight size={14} />
                        <span>详情</span>
                      {/if}
                    </button>

                    {#if expandedResources[resource.uri]}
                      <div
                        class="mt-3 pl-4 border-l-2 border-base-300 space-y-1"
                      >
                        <div class="text-xs">
                          <span class="text-base-content/60">URI:</span>
                          <span class="ml-2 font-mono text-primary break-all"
                            >{resource.uri}</span
                          >
                        </div>
                        {#if resource.mimeType}
                          <div class="text-xs">
                            <span class="text-base-content/60">MIME Type:</span>
                            <span class="ml-2 text-base-content"
                              >{resource.mimeType}</span
                            >
                          </div>
                        {/if}
                      </div>
                    {/if}
                  </TableBaseRow>
                </TableGroup>
              {/each}
            </div>
          {/if}
        {/if}
      {/if}
    {/if}
  </main>
</div>

<!-- 编辑弹窗 -->
<McpServerFormModal
  open={showEditModal}
  {server}
  onClose={closeEditModal}
  onSave={handleSaveServer}
/>

<!-- 删除确认弹窗 -->
<ConfirmModal
  bind:this={confirmModalRef}
  open={showDeleteConfirm}
  title="删除 MCP 服务器"
  message="确认要删除 <span class='font-medium'>{server?.displayName ||
    server?.name}</span> 吗？<br/><br/>此操作无法撤销。"
  confirmText="删除"
  cancelText="取消"
  confirmButtonStyle="danger"
  isLoading={mcpState.isLoading}
  autoCloseOnConfirm={false}
  onClose={() => (showDeleteConfirm = false)}
  onConfirm={confirmDelete}
  onCancel={() => {}}
/>

<!-- 禁用确认弹窗 -->
<ConfirmModal
  open={showDisableConfirm}
  title="关闭 MCP 服务器"
  message={relatedChatsCount > 0
    ? `检测到有 <span class='font-medium'>${relatedChatsCount}</span> 个会话正在使用 <span class='font-medium'>${server?.displayName || server?.name}</span>。<br/><br/>请选择要执行的操作：`
    : `确认关闭 <span class='font-medium'>${server?.displayName || server?.name}</span> 吗？`}
  actions={relatedChatsCount > 0
    ? [
        {
          label: "解除关联后关闭",
          style: "primary",
          onClick: handleDisableAndRemove
        },
        {
          label: "仅关闭 MCP",
          style: "danger",
          onClick: handleDisableWithoutRemove
        },
        {
          label: "取消",
          style: "secondary",
          onClick: handleCancelDisable
        }
      ]
    : undefined}
  confirmText="关闭"
  cancelText="取消"
  confirmButtonStyle="danger"
  onClose={() => (showDisableConfirm = false)}
  onConfirm={handleDisableWithoutRemove}
  onCancel={handleCancelDisable}
/>
