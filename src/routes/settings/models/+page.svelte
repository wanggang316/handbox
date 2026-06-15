<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    providerState,
    providerActions,
    providerStateActions,
    providerConfigs,
    getProviderIcon,
  } from "$lib/states/provider.svelte";
  import { LoaderCircle, Cpu } from "@lucide/svelte";
  import { TableGroup } from "$lib/components/ui/table";
  import StatusLabelRow from "$lib/components/ui/table/StatusLabelRow.svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import type { Provider } from "$lib/types/provider";
  import Button from "$lib/components/ui/Button.svelte";

  let showAddProviderModal = $state(false);

  onMount(async () => {
    try {
      // 并行加载供应商配置和供应商列表
      await Promise.all([
        providerActions.loadProviderConfigs(),
        providerActions.loadProviders()
      ]);
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

  // 监听模态框状态变化
  $effect(() => {
    if (!showAddProviderModal) {
      // 模态框关闭时，确保清理编辑状态
      providerStateActions.endEditProvider();
    }
  });

  function getProviderStatus(
    provider: Provider,
  ): "enabled" | "disabled" {
    return provider.enabled ? "enabled" : "disabled";
  }

  function getProviderStatusText(provider: Provider): string {
    return provider.enabled ? "已启用" : "已禁用";
  }
</script>

<div class="p-6 pr-8 pt-14 flex flex-col gap-y-4">

  <!-- 加载状态 -->
  {#if providerState.isLoading}
    <div class="flex items-center justify-center py-8">
      <LoaderCircle class="h-6 w-6 animate-spin text-base-content/60" />
      <span class="ml-2 text-sm text-base-content/70">正在加载供应商...</span>
    </div>
  {/if}

  <div class="rounded-xl overflow-hidden">
    <!-- 供应商列表 -->
    <TableGroup>
      <!-- 实际供应商 -->
      {#each providerState.providers as provider (provider.id)}
        <StatusLabelRow
          label={provider.name}
          iconSrc={getProviderIcon(provider)}
          icon={!getProviderIcon(provider)
            ? provider.name.charAt(0).toUpperCase()
            : undefined}
          isCustomProvider={![...providerConfigs.providers, ...providerConfigs.custom_providers].some(
            (t) => t.provider_type === provider.provider_type,
          )}
          status={getProviderStatus(provider)}
          statusText={getProviderStatusText(provider)}
          onclick={() => handleProviderClick(provider)}
        />
      {/each}

      <!-- 空状态 -->
      {#if !providerState.isLoading && providerState.providers.length === 0}
        <div class="p-8 text-center">
          <Cpu class="h-12 w-12 text-base-content/50 mx-auto mb-4" />
          <p class="text-base text-base-content/70 mb-4">
            添加 AI 供应商开始使用各种模型
          </p>
          <Button variant="primary" size="sm" onclick={handleAddProvider}>
            添加供应商
          </Button>
        </div>
      {/if}
    </TableGroup>
  </div>

  <!-- 添加供应商按钮 -->
  {#if providerState.providers.length > 0}
    <div>
      <Button variant="gray" size="sm" onclick={handleAddProvider}>
        添加其它供应商
      </Button>
    </div>
  {/if}
</div>

<!-- 添加供应商弹窗 -->
<AddProviderModal
  open={showAddProviderModal}
  onClose={() => showAddProviderModal = false}
/>
