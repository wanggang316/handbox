<script lang="ts">
  import Drawer from "../../ui/Drawer.svelte";
  import PromptSettings from "./Prompt.svelte";
  import ModelSelection from "./ModelSelection.svelte";
  import ModelParameters from "./ModelParameters.svelte";
  import McpSettings from "./Tools.svelte";
  import { providerActions } from "$lib/states/provider.svelte";
  import { mcpActions } from "$lib/states/mcp.svelte";
  import { currentChatModel } from "$lib/states/chat.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  // 检查当前模型是否支持工具调用
  const supportsTools = $derived(currentChatModel().model?.support_tools ?? false);

  let isRefreshing = $state(false);

  async function refreshChatSettingsData() {
    if (isRefreshing) {
      return;
    }

    isRefreshing = true;
    try {
      const [providersResult, mcpResult] = await Promise.allSettled([
        providerActions.loadProvidersWithModels(true),
        mcpActions.loadServers(true),
      ]);

      if (providersResult.status === "rejected") {
        console.error(
          "Failed to refresh providers and models:",
          providersResult.reason
        );
      }

      if (mcpResult.status === "rejected") {
        console.error("Failed to refresh MCP servers:", mcpResult.reason);
      }
    } finally {
      isRefreshing = false;
    }
  }

  $effect(() => {
    if (open) {
      // void refreshChatSettingsData();
    }
  });
</script>

<Drawer {open} title="聊天设置" {onClose}>
  <div class="flex flex-col gap-6 px-4 py-6 w-[360px]">
    <ModelSelection />
    <PromptSettings />
    <ModelParameters />
    {#if supportsTools}
      <McpSettings />
    {/if}
  </div>
</Drawer>
