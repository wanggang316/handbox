<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    providerState,
    providerActions,
    getProviderIcon,
    getPreProvider,
  } from "$lib/states/provider.svelte";
  import type { Provider, ProviderConfig } from "$lib/types/provider";
  import {
    Trash2,
    ChevronLeft,
    SquarePen,
    Settings2,
    ListChecks,
  } from "@lucide/svelte";
  import ModelSelectModal from "$lib/components/settings/ModelSelectModal.svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  let providerId = $state("");
  let showModelsModal = $state(false);
  let showDeleteConfirm = $state(false);
  let showEditModal = $state(false);
  
  let confirmModalRef: any;

  let isLoadingModels = $state(false);

  // 当前供应商
  let currentProvider = $state<Provider | null>(null);

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
      // 从全局状态中查找供应商
      const provider = providerState.providers.find((p) => p.id === providerId);

      if (provider) {
        currentProvider = provider;
        // 填充表单数据
        formData = {
          name: provider.name,
          enabled: provider.enabled,
        };
      } else {
        // 如果本地没有，先加载供应商列表
        await providerActions.loadProviders();
        const loadedProvider = providerState.providers.find(
          (p) => p.id === providerId,
        );

        if (loadedProvider) {
          currentProvider = loadedProvider;
          formData = {
            name: loadedProvider.name,
            enabled: loadedProvider.enabled,
          };
        } else {
          console.error("Provider not found:", providerId);
          // 跳转到供应商列表页
          goto("/settings/models");
        }
      }
    } catch (error) {
      console.error("Failed to load provider:", error);
    }
  }

  function handleEdit() {
    if (!currentProvider) return;
    showEditModal = true;
  }

  function handleCloseEdit() {
    showEditModal = false;
  }

  async function handleEditConfirm(event: CustomEvent<Provider>) {
    const updatedProvider = event.detail;
    
    // 更新本地的当前供应商数据
    currentProvider = updatedProvider;

    // 更新表单数据
    formData = {
      name: updatedProvider.name,
      enabled: updatedProvider.enabled,
    };

    // 重新加载供应商列表以确保数据同步
    await providerActions.loadProviders();
  }

  async function handleFetchModels() {
    if (!currentProvider) return;

    isLoadingModels = true;
    try {
      await providerActions.fetchProviderModels(currentProvider.id, true); // force refresh
      showModelsModal = true;
    } catch (error) {
      console.error("Failed to fetch models:", error);
      // 即使失败也显示模态框，让用户看到错误
      showModelsModal = true;
    } finally {
      isLoadingModels = false;
    }
  }

  async function handleToggleProvider(enabled: boolean) {
    console.log("handleToggleProvider", enabled);
    if (!currentProvider) return;

    formData.enabled = enabled; // 立即更新UI

    try {
      console.log("handleToggleProvider", currentProvider.id, enabled);
      await providerActions.toggleProvider(currentProvider.id, enabled);
      // 更新当前供应商状态
      currentProvider = { ...currentProvider, enabled };
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

  function handleConfigModel(model: any): void {
    // TODO: 实现模型配置功能
    console.log("Config model:", model);
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

    <div class="flex items-center mt-6">
      <div class="flex-1 text-text-primary text-base mx-2">模型列表</div>
      <Button
        on:click={handleFetchModels}
        variant="clear"
        size="sm"
        disabled={isLoadingModels || !currentProvider}
      >
        <ListChecks size={16} class={isLoadingModels ? "animate-spin" : ""} />
        {isLoadingModels ? "获取中..." : "管理模型"}
      </Button>
    </div>

    {#if currentProvider}
      {@const providerModels = providerState.availableModels.filter(
        (m) => m.provider_id === currentProvider?.id,
      )}
      {#if providerModels.length > 0}
        <TableGroup title={currentProvider.name}>
          {#each providerModels as model}
            <TableBaseRow label={model.name} py="2">
              <div class="flex flex-row items-center gap-2">
                <Toggle
                  checked={model.enabled}
                  onChange={(enabled) => {
                    if (currentProvider) {
                      providerActions.toggleModel(
                        currentProvider.id,
                        model.id,
                        enabled,
                      );
                    }
                  }}
                />
                <IconButton
                  icon={Settings2}
                  iconSize={16}
                  on:click={() => handleConfigModel(model)}
                  disabled={true}
                />
              </div>
            </TableBaseRow>
          {/each}
        </TableGroup>
      {:else}
        <div class="text-center text-sm py-8 text-gray-500">
          默认显示所有模型，可以通过“管理模型”按钮进行手动管理
        </div>
      {/if}
    {/if}
  </main>
</div>

<!-- 编辑供应商弹窗 -->
<AddProviderModal
  open={showEditModal}
  editProvider={currentProvider}
  on:close={handleCloseEdit}
  on:confirm={handleEditConfirm}
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
  onClose={() => showDeleteConfirm = false}
  onConfirm={confirmDelete}
  onCancel={() => {}}
/>
