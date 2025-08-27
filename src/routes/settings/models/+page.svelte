<script lang="ts">
  import { onMount } from "svelte";
  import { providers, providerActions } from "$lib/stores/provider";
  import { Plus, Cpu } from "@lucide/svelte";
  import { TableGroup } from "$lib/components/ui/table";
  import StatusLabelRow from "$lib/components/ui/table/StatusLabelRow.svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import type { Provider, ProviderConfig } from "$lib/types/provider";
  import Button from "$lib/components/ui/Button.svelte";

  let showAddProviderModal = $state(false);

  // 预定义供应商列表
  const presetProviders = [
    {
      name: "OpenAI",
      type: "openai",
      iconSrc: "/logo-openai.png",
      enabled: true,
    },
    {
      name: "Anthropic",
      type: "anthropic",
      iconSrc: "/logo-anthropic.png",
      enabled: false,
    },
    {
      name: "Google AI",
      type: "google",
      iconSrc: "/logo-google.png",
      enabled: false,
    },
    {
      name: "DeepSeek",
      type: "deepseek",
      iconSrc: "/logo-deepseek.png",
      enabled: false,
    },
    {
      name: "OpenRouter",
      type: "openrouter",
      iconSrc: "/logo-openrouter.png",
      enabled: false,
    },
  ];

  onMount(async () => {
    try {
      await providerActions.loadProviders();
    } catch (error) {
      console.error("Failed to load providers:", error);
    }
  });

  function handleProviderClick(provider: any) {
    // 跳转到供应商配置页面
    window.location.href = `/settings/models/provider/${provider.id || provider.type}`;
  }

  function handleAddProvider() {
    showAddProviderModal = true;
  }

  async function handleCreateProvider(event: CustomEvent<ProviderConfig>) {
    const config = event.detail;
    try {
      await providerActions.createProvider(config);
      // showAddProviderModal = false;
      // 跳转到新创建的供应商配置页面
      // TODO: 获取新创建的供应商ID
    } catch (error) {
      console.error("Failed to create provider:", error);
    }
  }

  function handleCloseAddProvider() {
    showAddProviderModal = false;
  }
</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <div class="rounded-[20px] overflow-hidden">
    <!-- 供应商列表 -->
    <TableGroup>
      <!-- 预定义供应商 -->
      {#each presetProviders as provider}
        <StatusLabelRow
          label={provider.name}
          iconSrc={provider.iconSrc}
          status={provider.enabled ? "enabled" : "disabled"}
          statusText={provider.enabled ? "已开启" : "未开启"}
          onclick={() => handleProviderClick(provider)}
        />
      {/each}

      <!-- 自定义供应商 -->
      {#each $providers as provider}
        <StatusLabelRow
          label={provider.name}
          icon={provider.name.charAt(0).toUpperCase()}
          isCustomProvider={true}
          status={provider.enabled ? "enabled" : "disabled"}
          statusText={provider.enabled ? "已配置" : "未配置"}
          onclick={() => handleProviderClick(provider)}
        />
      {/each}
    </TableGroup>
  </div>

  <div>
    <Button variant="gray" size="sm" on:click={handleAddProvider}
      >添加供应商</Button
    >
  </div>
</div>

<!-- 添加供应商弹窗 -->
<AddProviderModal
  open={showAddProviderModal}
  on:close={handleCloseAddProvider}
  on:confirm={handleCreateProvider}
/>
