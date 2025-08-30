<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { providerState, providerActions, getEnabledProviders, preProviders, getProviderIcon } from "$lib/states/provider.svelte";
  import { Plus, Cpu, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import { TableGroup } from "$lib/components/ui/table";
  import StatusLabelRow from "$lib/components/ui/table/StatusLabelRow.svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import type { Provider, ProviderConfig } from "$lib/types/provider";
  import Button from "$lib/components/ui/Button.svelte";

  let showAddProviderModal = $state(false);


  onMount(async () => {
    try {
      await providerActions.loadProviders();
    } catch (error) {
      console.error("Failed to load providers:", error);
    }
  });

  function handleProviderClick(provider: Provider) {
    // 跳转到供应商配置页面
    goto(`/settings/models/provider/${provider.id}`);
  }

  function handleAddProvider() {
    showAddProviderModal = true;
  }

  async function handleCreateProvider(event: CustomEvent<ProviderConfig>) {
    const config = event.detail;
    try {
      const newProvider = await providerActions.createProvider(config);
      showAddProviderModal = false;
      // 跳转到新创建的供应商配置页面
      goto(`/settings/models/provider/${newProvider.id}`);
    } catch (error) {
      console.error("Failed to create provider:", error);
    }
  }

  function handleCloseAddProvider() {
    showAddProviderModal = false;
  }

  function getProviderStatus(provider: Provider): "enabled" | "disabled" | "idle" | "error" {
    // if (provider.enabled) return "enabled";
    if (provider.status === "error") return "error";
    if (provider.status === "disabled") return "disabled";
    if (provider.status === "idle") return "idle";
    return "enabled";
  }

  function getProviderStatusText(provider: Provider): string {
    switch (provider.status) {
      case "enabled": return "已开启";
      case "error": return "错误";
      case "disabled": return "未开启";
      case "idle": return "未配置";
      default: return "未知";
    }
  }
</script>

<div class="p-6 pr-8 pt-14 flex flex-col gap-y-4">
  <!-- 错误提示 -->
  {#if providerState.error}
    <div class="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-4 flex items-start gap-3">
      <TriangleAlert class="h-5 w-5 text-red-600 dark:text-red-400 mt-0.5 flex-shrink-0" />
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-red-800 dark:text-red-200">加载失败</p>
        <p class="text-sm text-red-700 dark:text-red-300 mt-1">{providerState.error}</p>
        <button 
          class="text-sm text-red-700 dark:text-red-300 underline mt-2 hover:no-underline"
          onclick={() => providerActions.clearError()}
        >
          忽略
        </button>
      </div>
    </div>
  {/if}

  <!-- 加载状态 -->
  {#if providerState.isLoading}
    <div class="flex items-center justify-center py-8">
      <LoaderCircle class="h-6 w-6 animate-spin text-gray-400" />
      <span class="ml-2 text-sm text-gray-500">正在加载供应商...</span>
    </div>
  {/if}

  <div class="rounded-[20px] overflow-hidden">
    <!-- 供应商列表 -->
    <TableGroup>
      <!-- 实际供应商 -->
      {#each providerState.providers as provider (provider.id)}
        <StatusLabelRow
          label={provider.name}
          iconSrc={getProviderIcon(provider)}
          icon={!getProviderIcon(provider) ? provider.name.charAt(0).toUpperCase() : undefined}
          isCustomProvider={!preProviders.some(t => t.provider_type === provider.provider_type)}
          status={getProviderStatus(provider)}
          statusText={getProviderStatusText(provider)}
          onclick={() => handleProviderClick(provider)}
        />
      {/each}

      <!-- 空状态 -->
      {#if !providerState.isLoading && providerState.providers.length === 0}
        <div class="p-8 text-center">
          <Cpu class="h-12 w-12 text-gray-400 mx-auto mb-4" />
          <h3 class="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
            还没有供应商
          </h3>
          <p class="text-gray-500 dark:text-gray-400 mb-4">
            添加 AI 供应商开始使用各种模型
          </p>
          <Button variant="primary" size="sm" on:click={handleAddProvider}>
            <Plus class="h-4 w-4 mr-2" />
            添加供应商
          </Button>
        </div>
      {/if}
    </TableGroup>
  </div>

  <!-- 添加供应商按钮 -->
  {#if providerState.providers.length > 0}
    <div>
      <Button variant="gray" size="sm" on:click={handleAddProvider}>
        添加其它供应商
      </Button>
    </div>
  {/if}
</div>

<!-- 添加供应商弹窗 -->
<AddProviderModal
  open={showAddProviderModal}
  on:close={handleCloseAddProvider}
  on:confirm={handleCreateProvider}
/>
