<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    providerState,
    providerActions,
    providerStateActions,
    getProviderIcon,
  } from "$lib/states/provider.svelte";
  import type { Provider, ProviderConfig } from "$lib/types/provider";
  import { Trash2, ChevronLeft, SquarePen, Heart, Star } from "@lucide/svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";

  let providerId = $state("");
  let showDeleteConfirm = $state(false);
  let showEditModal = $state(false);

  let confirmModalRef: any;

  // 使用统一的当前供应商状态
  const currentProvider = $derived(providerState.currentProvider);

  // 配置表单
  let formData = $state({
    name: "",
    enabled: false,
  });

  // 获取预定义供应商信息
  let providerIcon = $derived(
    currentProvider ? getProviderIcon(currentProvider) : null,
  );

  onMount(() => {
    providerId = $page.params.id || "";
    loadProvider();
  });

  async function loadProvider() {
    if (!providerId) return;

    try {
      // 尝试从全局状态中设置当前供应商
      let provider = providerStateActions.setCurrentProviderById(providerId);

      if (!provider) {
        // 如果本地没有，先加载供应商配置和列表
        await Promise.all([
          providerActions.loadProviderConfigs(),
          providerActions.loadProviders(),
        ]);
        provider = providerStateActions.setCurrentProviderById(providerId);
      }

      if (provider) {
        // 填充表单数据
        formData = {
          name: provider.name,
          enabled: provider.enabled,
        };

        // 自动获取模型列表
        try {
          await providerActions.fetchProviderModels(provider.id, false);
        } catch (error) {
          console.error("Failed to fetch models:", error);
        }
      } else {
        console.error("Provider not found:", providerId);
        // 跳转到供应商列表页
        goto("/settings/models");
      }
    } catch (error) {
      console.error("Failed to load provider:", error);
    }
  }

  function handleEdit() {
    if (!currentProvider) return;
    providerStateActions.startEditProvider(currentProvider);
    showEditModal = true;
  }

  // 监听当前供应商变化，更新表单数据
  $effect(() => {
    if (currentProvider) {
      formData = {
        name: currentProvider.name,
        enabled: currentProvider.enabled,
      };
    }
  });

  async function handleToggleProvider(enabled: boolean) {
    console.log("handleToggleProvider", enabled);
    if (!currentProvider) return;

    formData.enabled = enabled; // 立即更新UI

    try {
      console.log("handleToggleProvider", currentProvider.id, enabled);
      await providerActions.toggleProvider(currentProvider.id, enabled);
      // 更新当前供应商状态
      providerStateActions.updateCurrentProvider({
        ...currentProvider,
        enabled,
      });
      console.log(`Provider ${enabled ? "enabled" : "disabled"} successfully`);
    } catch (error) {
      console.error("Failed to toggle provider:", error);
      // 发生错误时回滚UI状态
      formData.enabled = !enabled;
    }
  }

  async function handleDelete() {
    if (!currentProvider) return;
    showDeleteConfirm = true;
  }

  async function confirmDelete() {
    if (!currentProvider) return;

    try {
      await providerActions.deleteProvider(currentProvider.id);
      console.log("Provider deleted successfully");
      goto("/settings/models");
    } catch (error) {
      console.error("Delete failed:", error);
      // 删除失败时触发关闭动画
      confirmModalRef?.modalRef?.handleClose();
    }
  }

  function handleBack() {
    goto("/settings/models");
  }
</script>

{#snippet iconSnippet()}
  {#if providerIcon}
    <img
      src={providerIcon}
      alt="{currentProvider?.name || 'Provider'} logo"
      class="w-6 h-6 object-contain"
    />
  {:else if currentProvider}
    <div
      class="w-6 h-6 bg-gray-300 rounded flex items-center justify-center text-xs font-bold"
    >
      {currentProvider.name.charAt(0).toUpperCase()}
    </div>
  {/if}
{/snippet}

<!-- 粘性导航栏 - 在右侧主体区域内固定 -->
<div class="flex flex-col h-screen">
  <header class=" text-white py-2 px-4 flex-shrink-0">
    <CircleButton
      icon={ChevronLeft}
      iconSize={22}
      ariaLabel="返回"
      size="w-8 h-8"
      bgColor="bg-bg-secondary"
      hoverColor="hover:bg-bg-hover"
      textColor="text-text-primary"
      customClass="hover:text-text-secondary z-10004 relative"
      on:click={handleBack}
    />
  </header>

  <!-- 主要内容区域 -->
  <main class="flex-grow overflow-y-auto p-6 pr-8">
    {#if currentProvider}
      <TableGroup>
        <TableBaseRow label={currentProvider.name} icon={iconSnippet}>
          <div class="flex flex-row items-center gap-4">
            <IconButton icon={SquarePen} on:click={handleEdit} />

            <IconButton icon={Trash2} on:click={handleDelete} />

            <Toggle
              checked={formData.enabled}
              onChange={handleToggleProvider}
            />
          </div>
        </TableBaseRow>
      </TableGroup>
    {/if}

    <div class="flex items-center mt-6 mb-2">
      <div class="flex-1 text-text-primary text-base mx-2">模型列表</div>
    </div>

    {#if currentProvider}
      {@const providerModels = providerState.currentModels}
      {#if providerModels.length > 0}
        <div class="bg-bg-secondary rounded-xl overflow-hidden">
          <!-- Table Headers -->
          <div
            class="flex flex-row items-center gap-4 px-4 py-2 bg-bg-hover border-b border-border text-xs font-medium text-text-primary"
          >
            <div class="flex-1">Name</div>
            <div class="text-center w-16">Enabled</div>
            <div class="text-center w-16">Favorite</div>
          </div>

          <!-- Model List -->
          <div class="bg-bg-primary">
            {#each providerModels as model, index}
              <div
                class="flex flex-row items-center gap-4 px-4 py-1 {index % 2 === 0 ? 'bg-bg-primary' : 'bg-bg-secondary'} hover:bg-bg-hover"
              >
                <!-- Model Name -->
                <div class="flex items-center flex-1">
                  <span class="text-text-primary text-xs">{model.name}</span>
                </div>

                <!-- Enabled Toggle -->

                <div class="flex items-center justify-center w-16">
                  <input
                    type="checkbox"
                    bind:checked={model.enabled}
                    onchange={(e) => {
                      if (currentProvider) {
                        providerActions.toggleModel(
                          currentProvider.id,
                          model.id,
                          (e.currentTarget as HTMLInputElement).checked,
                        );
                      }
                    }}
                    class="w-4 h-4 text-accent bg-bg-primary border-border rounded focus:ring-accent focus:ring-2"
                  />
                </div>

                <div class="flex items-center justify-center w-16">
                  <button
                    onclick={() => {
                      if (currentProvider) {
                        providerActions.toggleModelFavorite(
                          currentProvider.id,
                          model.id,
                          !model.favorite,
                        );
                      }
                    }}
                    class="p-1 rounded hover:bg-bg-hover transition-colors"
                    aria-label={model.favorite
                      ? "Remove from favorites"
                      : "Add to favorites"}
                  >
                    <Star
                      size={16}
                      class={model.favorite
                        ? "text-text-primary fill-current"
                        : "text-text-secondary hover:text-red-400"}
                    />
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="text-center text-sm py-8 text-gray-500">
          暂无模型数据，请检查供应商配置或网络连接
        </div>
      {/if}
    {/if}
  </main>
</div>

<!-- 编辑供应商弹窗 -->
<AddProviderModal
  open={showEditModal}
  onClose={() => (showEditModal = false)}
/>

<!-- 删除确认弹窗 -->
<ConfirmModal
  bind:this={confirmModalRef}
  open={showDeleteConfirm}
  title="删除供应商"
  message="确认要删除 <span class='font-medium'>{currentProvider?.name}</span> 吗？"
  confirmText="删除"
  cancelText="取消"
  confirmButtonStyle="danger"
  isLoading={providerState.isLoading}
  autoCloseOnConfirm={false}
  onClose={() => (showDeleteConfirm = false)}
  onConfirm={confirmDelete}
  onCancel={() => {}}
/>
