<script lang="ts">
  import McpServerTextEditModal from "$lib/components/settings/McpServerTextEditModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { ChevronsUpDown } from "@lucide/svelte";

  let showMcpServerTextEditModal = $state(false);

  let expandedStates = $state<Record<string, boolean>>({});

  let mcpList = [
    {
      name: "@fetch",
      enabled: false,
      status: "disabled",
      tools: [
        {
          name: "fetch",
          enabled: false,
        },
        {
          name: "get",
          enabled: false,
        },
        {
          name: "query",
          enabled: false,
        },
        {
          name: "fetch",
          enabled: false,
        },
        {
          name: "get",
          enabled: false,
        },
        {
          name: "query",
          enabled: false,
        },
      ],
    },
    {
      name: "@smithery-ai/server-sequential-thinking",
      enabled: true,
      status: "logout",
      tools: [
        {
          name: "sequential-thinking",
          enabled: false,
        },
      ],
    },
    {
      name: "@upstash/context7-mcp",
      enabled: false,
      status: "normal",
      tools: [
        {
          name: "context7",
          enabled: false,
        },
      ],
    },
  ];

  function handleManageMcp() {
    showMcpServerTextEditModal = true;
  }

  async function handleConfirmMcpServerTextEdit(
    event: CustomEvent<string>,
  ) {
    const mcpJson = event.detail;
    try {
      // await providerActions.createProvider(config);
      // showMcpServerTextEditModal = false; // 让 Modal 组件自己处理关闭动画
    } catch (error) {
      console.error("Failed to create mcp server:", error);
    }
  }

  function handleCloseMcpServerTextEditModal() {
    showMcpServerTextEditModal = false;
  }
</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup>
    {#each mcpList as mcp}
      {#snippet toggleSnippet()}
        <Toggle checked={mcp.enabled} />
      {/snippet}

      <TableBaseRow
        label={mcp.name}
        layout="vertical"
        rightContent={toggleSnippet}
      >
        <div class="flex flex-col items-start gap-2">
          <button
            onclick={() =>
              (expandedStates[mcp.name] = !expandedStates[mcp.name])}
            class="text-xs text-base-content/70 hover:text-base-content"
          >
            <div class="flex flex-row items-center gap-1">
              <span>tools</span>
              <ChevronsUpDown class="w-3 h-3" />
            </div>
          </button>

          {#if expandedStates[mcp.name]}
            <div class="flex flex-row items-center gap-2">
              {#each mcp.tools as tool}
                <div
                  class="flex flex-row items-center gap-1 bg-base-200 rounded-sm py-0.5 px-1 text-xs"
                >
                  {tool.name}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </TableBaseRow>
    {/each}
  </TableGroup>

  <div>
    <Button variant="gray" size="sm" on:click={handleManageMcp}
      >管理MCP服务器</Button
    >
  </div>
</div>

<McpServerTextEditModal
  open={showMcpServerTextEditModal}
  on:close={handleCloseMcpServerTextEditModal}
  on:confirm={handleConfirmMcpServerTextEdit}
/>
