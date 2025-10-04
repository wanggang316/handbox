<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import Tabs from '$lib/components/ui/Tabs.svelte';
  import CircleButton from '$lib/components/ui/CircleButton.svelte';
  import TableGroup from '$lib/components/ui/table/TableGroup.svelte';
  import TableBaseRow from '$lib/components/ui/table/TableBaseRow.svelte';
  import IconButton from '$lib/components/ui/IconButton.svelte';
  import Toggle from '$lib/components/ui/Toggle.svelte';
  import { mcpState, mcpActions } from '$lib/states/mcp.svelte';
  import { updateToolEnabled } from '$lib/api';
  import type { McpServer } from '$lib/types';
  import { ChevronLeft, RefreshCw, SquarePen, Trash2, ChevronDown, ChevronRight } from '@lucide/svelte';

  let serverId = $state('');
  let activeTab = $state('tools');
  let isRefreshing = $state(false);

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
    serverId = $page.params.id || '';
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
        console.error('MCP server not found:', serverId);
        goto('/settings/mcp');
      }
    } catch (error) {
      console.error('Failed to load MCP server:', error);
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

  async function handleToggle(enabled: boolean) {
    if (!server) return;

    formData.enabled = enabled; // 立即更新UI

    try {
      await mcpActions.toggleServer({ serverId: server.id, enabled });
      console.log(`MCP server ${enabled ? 'enabled' : 'disabled'} successfully`);
    } catch (error) {
      console.error('Failed to toggle MCP server:', error);
      // 发生错误时回滚UI状态
      formData.enabled = !enabled;
    }
  }

  async function handleRefresh() {
    if (!server || isRefreshing) return;

    isRefreshing = true;
    try {
      await mcpActions.refreshServer({ serverId: server.id });
    } catch (error) {
      console.error('Failed to refresh MCP server:', error);
    } finally {
      isRefreshing = false;
    }
  }

  function handleEdit() {
    // TODO: 实现编辑功能
    console.log('Edit MCP server:', server);
  }

  async function handleDelete() {
    if (!server) return;

    const confirmed = confirm(`确定要删除 MCP 服务器 "${server.displayName || server.name}" 吗？\n\n此操作无法撤销。`);
    if (!confirmed) return;

    try {
      await mcpActions.deleteServer(server.id);
      console.log('MCP server deleted successfully');
      goto('/settings/mcp');
    } catch (error) {
      console.error('Failed to delete MCP server:', error);
      alert('删除失败: ' + (error as Error).message);
    }
  }

  function handleBack() {
    goto('/settings/mcp');
  }

  const connectionTypeLabel = $derived(() => {
    if (!server) return '';
    switch (server.connectionType) {
      case 'stdio':
        return '标准输入输出 (stdio)';
      case 'sse':
        return '服务器发送事件 (SSE)';
      case 'http':
        return 'HTTP 端点';
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
        enabled
      });

      // 手动更新状态
      await mcpActions.loadServers();
    } catch (error) {
      console.error('Failed to update tool enabled status:', error);
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
      bgColor="bg-base-200"
      hoverColor="hover:bg-base-300"
      textColor="text-base-content"
      customClass="hover:text-base-content/80 z-10004 relative"
      onclick={handleBack}
    />
  </header>

  <!-- 主要内容区域 -->
  <main class="flex-grow overflow-y-auto p-6 pr-8">
    {#if server}
      <!-- 基本信息卡片 -->
      <TableGroup>
        <TableBaseRow label={server.displayName || server.name}>
          <div class="flex flex-row items-center gap-4">
            <IconButton icon={SquarePen} on:click={handleEdit} />
            <IconButton icon={Trash2} on:click={handleDelete} />
            <IconButton
              icon={RefreshCw}
              on:click={handleRefresh}
              disabled={!server.enabled || isRefreshing}
              customClass={isRefreshing ? 'animate-spin' : ''}
            />
            <Toggle checked={formData.enabled} onChange={handleToggle} />
          </div>
        </TableBaseRow>
      </TableGroup>

      <!-- Tab 导航 -->
      <Tabs
        value={activeTab}
        items={[
          { value: 'tools', label: '工具' },
          { value: 'prompts', label: '提示' },
          { value: 'resources', label: '资源' }
        ]}
        onChange={(val) => { activeTab = val; }}
      />

      <!-- Tab 内容 -->
      {#if activeTab === 'tools'}
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
                        <p class="text-xs text-base-content/70">{tool.description}</p>
                      {/if}
                    </div>
                    <Toggle
                      checked={server.enabledTools.includes(tool.name)}
                      onChange={(enabled) => handleToolToggle(tool.name, enabled)}
                    />
                  </div>

                  {#if tool.inputSchema && typeof tool.inputSchema === 'object' && 'properties' in tool.inputSchema && Object.keys(tool.inputSchema.properties || {}).length > 0}
                    <button
                      class="flex items-center gap-1 text-xs text-base-content/80 hover:text-base-content/80 mt-2"
                      onclick={() => toggleTool(tool.name)}
                    >
                      {#if expandedTools[tool.name]}
                        <ChevronDown size={12} />
                        <span>参数</span>
                      {:else}
                        <ChevronRight size={12} />
                        <span>参数 ({Object.keys(tool.inputSchema.properties || {}).length})</span>
                      {/if}
                    </button>

                    {#if expandedTools[tool.name]}
                      <div class="mt-3 pl-4 border-l-2 border-base-300">
                        <div class="space-y-2">
                          {#each Object.entries(tool.inputSchema.properties || {}) as [key, value]}
                            {@const propValue = value as { type?: string; description?: string }}
                            {@const requiredFields = (tool.inputSchema as { required?: string[] })?.required || []}
                            {@const isRequired = requiredFields.includes(key)}
                            <div class="text-xs">
                              <span class="font-mono text-primary">{key}</span>
                              {#if isRequired}
                                <span class="text-error ml-1">*</span>
                              {/if}
                              {#if propValue.type}
                                <span class="text-base-content/60 ml-1">({propValue.type})</span>
                              {/if}
                              {#if propValue.description}
                                <span class="text-base-content/70 ml-2">- {propValue.description}</span>
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
      {:else if activeTab === 'prompts'}
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
                    <p class="text-xs text-base-content/70 mb-3">{prompt.description}</p>
                  {/if}

                  {#if prompt.arguments.length > 0}
                    <button
                      class="flex items-center gap-1 text-xs text-primary hover:text-primary/80 mt-2"
                      onclick={() => togglePrompt(prompt.name)}
                    >
                      {#if expandedPrompts[prompt.name]}
                        <ChevronDown size={14} />
                        <span>隐藏参数</span>
                      {:else}
                        <ChevronRight size={14} />
                        <span>显示参数 ({prompt.arguments.length})</span>
                      {/if}
                    </button>

                    {#if expandedPrompts[prompt.name]}
                      <div class="mt-3 pl-4 border-l-2 border-base-300">
                        <div class="space-y-2">
                          {#each prompt.arguments as arg}
                            <div class="text-xs">
                              <span class="font-mono text-primary">{arg.name}</span>
                              {#if arg.required}
                                <span class="text-error ml-1">*</span>
                              {/if}
                              {#if arg.description}
                                <span class="text-base-content/70 ml-2">- {arg.description}</span>
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
      {:else if activeTab === 'resources'}
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
                    <p class="text-xs text-base-content/70 mb-3">{resource.description}</p>
                  {/if}

                  <button
                    class="flex items-center gap-1 text-xs text-primary hover:text-primary/80 mt-2"
                    onclick={() => toggleResource(resource.uri)}
                  >
                    {#if expandedResources[resource.uri]}
                      <ChevronDown size={14} />
                      <span>隐藏详情</span>
                    {:else}
                      <ChevronRight size={14} />
                      <span>显示详情</span>
                    {/if}
                  </button>

                  {#if expandedResources[resource.uri]}
                    <div class="mt-3 pl-4 border-l-2 border-base-300 space-y-1">
                      <div class="text-xs">
                        <span class="text-base-content/60">URI:</span>
                        <span class="ml-2 font-mono text-primary break-all">{resource.uri}</span>
                      </div>
                      {#if resource.mimeType}
                        <div class="text-xs">
                          <span class="text-base-content/60">MIME Type:</span>
                          <span class="ml-2 text-base-content">{resource.mimeType}</span>
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
  </main>
</div>
