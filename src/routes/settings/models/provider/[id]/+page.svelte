<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    providerState,
    providerActions,
    providerStateActions,
    getProviderIcon,
    providerConfigs,
    isCustomProviderType,
  } from "$lib/states/provider.svelte";
  import type { Model } from "$lib/types/provider";
  import {
    Trash2,
    ChevronLeft,
    SquarePen,
    Star,
    Info,
    RefreshCw,
    Plus,
    Eye as EyeIcon,
  } from "@lucide/svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import ModelInfoModal from "$lib/components/settings/ModelInfoModal.svelte";
  import { countChatsUsingProvider } from "$lib/api/provider";
  import { countChatsUsingModel } from "$lib/api/model";

  let providerId = $state("");
  let showDeleteConfirm = $state(false);
  let showDisableConfirm = $state(false);
  let showModelDisableConfirm = $state(false);
  let showEditModal = $state(false);
  let showModelInfo = $state(false);
  let selectedModel = $state<Model | null>(null);
  let modelToDisable = $state<Model | null>(null);
  let relatedChatsCount = $state(0);

  let confirmModalRef: any;

  // 使用统一的当前供应商状态
  const currentProvider = $derived(providerState.currentProvider);

  // 自定义供应商（openai-compatible / anthropic-compatible）：端点不在 hand-ai
  // 目录中，模型需手动添加。
  const isCustom = $derived(
    !!currentProvider && isCustomProviderType(currentProvider.provider_type)
  );

  // 手动添加模型的内联表单状态
  let showAddModel = $state(false);
  let newModelId = $state("");
  let addingModel = $state(false);
  let addModelError = $state("");

  // 配置表单
  let formData = $state({
    name: "",
    enabled: false,
  });

  // 获取预定义供应商信息
  let providerIcon = $derived(
    currentProvider ? getProviderIcon(currentProvider) : null
  );

  const isRefreshing = $derived(
    !!(
      currentProvider?.id &&
      providerState.isFetchingModels === currentProvider.id
    )
  );

  onMount(() => {
    providerId = $page.params.id || "";
    loadProvider();
  });

  async function loadProvider() {
    if (!providerId) return;

    try {
      // 确保供应商配置模板已加载（isCustom 依赖 custom_providers）。
      if (providerConfigs.custom_providers.length === 0) {
        await providerActions.loadProviderConfigs();
      }

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
        if (provider.id) {
          try {
            await providerActions.fetchProviderModels(provider.id, false);
          } catch (error) {
            console.error("Failed to fetch models:", error);
          }
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

  async function handleToggleProviderBefore(
    enabled: boolean,
    previous: boolean
  ) {
    if (!currentProvider || !currentProvider.id) return true;

    if (!enabled && previous && currentProvider.enabled) {
      try {
        const count = await countChatsUsingProvider(currentProvider.id);
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

  async function handleToggleProvider(enabled: boolean) {
    if (!currentProvider || !currentProvider.id) return;
    await performProviderToggle(enabled);
  }

  async function performProviderToggle(enabled: boolean) {
    if (!currentProvider) return;

    // 乐观更新UI
    const previousState = formData.enabled;
    formData.enabled = enabled;

    try {
      if (!currentProvider.id) {
        throw new Error("Provider ID is undefined");
      }
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
      formData.enabled = previousState;
    }
  }

  async function confirmDisableProvider() {
    await performProviderToggle(false);
    showDisableConfirm = false;
  }

  function cancelDisableProvider() {
    if (currentProvider) {
      formData.enabled = currentProvider.enabled;
    }
    showDisableConfirm = false;
    // 保持当前 UI 状态不变
  }

  async function handleToggleModel(model: Model, enabled: boolean) {
    if (!currentProvider) return;

    // 如果是禁用操作，需要检查关联的聊天
    if (!enabled && model.enabled) {
      try {
        const count = await countChatsUsingModel(model.id);
        relatedChatsCount = count;
        modelToDisable = model;
        showModelDisableConfirm = true;
      } catch (error) {
        console.error("Failed to count related chats:", error);
        // 如果检查失败，仍然允许禁用
        performModelToggle(model, enabled);
      }
    } else {
      // 启用操作直接执行
      performModelToggle(model, enabled);
    }
  }

  async function performModelToggle(model: Model, enabled: boolean) {
    if (!currentProvider || !currentProvider.id) return;

    try {
      await providerActions.toggleModel(currentProvider.id, model.id, enabled);
    } catch (error) {
      console.error("Failed to toggle model:", error);
    }
  }

  async function confirmDisableModel() {
    if (modelToDisable) {
      await performModelToggle(modelToDisable, false);
      showModelDisableConfirm = false;
      modelToDisable = null;
    }
  }

  function cancelDisableModel() {
    showModelDisableConfirm = false;
    modelToDisable = null;
  }

  async function handleDelete() {
    if (!currentProvider) return;
    showDeleteConfirm = true;
  }

  function openModelInfo(model: Model) {
    selectedModel = model;
    showModelInfo = true;
  }

  function closeModelInfo() {
    showModelInfo = false;
    selectedModel = null;
  }

  async function refreshModels() {
    if (!currentProvider?.id) return;

    try {
      await providerActions.fetchProviderModels(currentProvider.id, true);
    } catch (error) {
      console.error("Failed to refresh models", error);
    }
  }

  async function handleAddModel() {
    if (!currentProvider?.id) return;
    const id = newModelId.trim();
    if (!id) return;

    addingModel = true;
    addModelError = "";
    try {
      await providerActions.addModel(currentProvider.id, id);
      newModelId = "";
      showAddModel = false;
    } catch (error) {
      addModelError =
        error instanceof Error ? error.message : "添加模型失败";
    } finally {
      addingModel = false;
    }
  }

  function cancelAddModel() {
    showAddModel = false;
    newModelId = "";
    addModelError = "";
  }

  async function confirmDelete() {
    if (!currentProvider || !currentProvider.id) return;

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
      class="w-6 h-6 bg-base-300 rounded flex items-center justify-center text-xs font-bold"
    >
      {currentProvider.name.charAt(0).toUpperCase()}
    </div>
  {/if}
{/snippet}

<!-- 粘性导航栏 - 在右侧主体区域内固定 -->
<div class="flex flex-col h-screen">
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
    {#if currentProvider}
      <TableGroup>
        <TableBaseRow label={currentProvider.name} icon={iconSnippet}>
          <div class="flex flex-row items-center gap-4">
            <IconButton icon={SquarePen} onclick={handleEdit} />

            <IconButton
              icon={RefreshCw}
              ariaLabel="刷新模型列表"
              onclick={refreshModels}
              disabled={isRefreshing}
              customClass={`transition-transform ${isRefreshing ? "animate-spin text-primary" : ""}`}
            />

            <IconButton icon={Trash2} onclick={handleDelete} />

            <Toggle
              checked={formData.enabled}
              onChangeBefore={handleToggleProviderBefore}
              onChange={handleToggleProvider}
            />
          </div>
        </TableBaseRow>
      </TableGroup>
    {/if}

    <div class="flex items-center mt-6 mb-2">
      <div class="flex-1 text-base-content text-base mx-2">模型列表</div>
      {#if isCustom}
        <button
          onclick={() => (showAddModel = !showAddModel)}
          class="flex items-center gap-1 px-2 py-1 mr-2 rounded-md text-xs bg-base-300 text-base-content border border-[var(--hairline)] hover:bg-base-300/80"
        >
          <Plus size={14} />
          添加模型
        </button>
      {/if}
    </div>

    {#if isCustom && showAddModel}
      <div
        class="flex items-center gap-2 mb-3 px-2 py-2 bg-base-200 rounded-lg"
      >
        <input
          type="text"
          bind:value={newModelId}
          placeholder="输入 model id，例如 llama-3.1-8b"
          onkeydown={(e) => {
            if (e.key === "Enter") handleAddModel();
            if (e.key === "Escape") cancelAddModel();
          }}
          class="flex-1 px-3 py-1.5 text-sm border border-[var(--hairline)] bg-base-100 rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
        />
        <button
          onclick={handleAddModel}
          disabled={addingModel || !newModelId.trim()}
          class="px-3 py-1.5 text-sm rounded-md bg-primary text-primary-content disabled:opacity-50"
        >
          {addingModel ? "添加中…" : "添加"}
        </button>
        <button
          onclick={cancelAddModel}
          class="px-3 py-1.5 text-sm rounded-md bg-base-300 text-base-content hover:bg-base-300/80"
        >
          取消
        </button>
      </div>
      {#if addModelError}
        <div class="text-error text-xs mb-2 px-2">{addModelError}</div>
      {/if}
    {/if}

    {#if currentProvider}
      {@const providerModels = providerState.currentModels}
      {#if providerModels.length > 0}
        <div class="bg-base-200 rounded-lg overflow-hidden">
          <!-- Table Headers -->
          <div
            class="flex flex-row items-center gap-4 px-4 py-2 bg-base-300 border-b border-base-300 text-xs font-medium text-base-content"
          >
            <div class="flex-1">Name</div>
            <div class="text-center w-16">Enabled</div>
            <div class="text-center w-16">Favorite</div>
            <div class="text-center w-14">Info</div>
          </div>

          <!-- Model List -->
          <div class="bg-base-100">
            {#each providerModels as model, index}
              <div
                class="flex flex-row items-center gap-4 px-4 py-1 {index % 2 ===
                0
                  ? 'bg-base-100'
                  : 'bg-base-200'} hover:bg-base-300"
              >
                <!-- Model Name -->
                <div class="flex items-center flex-1 gap-2">
                  <span class="text-base-content text-xs">{model.name}</span>
                  {#if model.support_image}
                    <span title="支持图片生成">
                      <EyeIcon size={14} class="text-info" />
                    </span>
                  {/if}
                </div>

                <!-- Enabled Toggle -->

                <div class="flex items-center justify-center w-16">
                  <input
                    type="checkbox"
                    checked={model.enabled}
                    onchange={(e) => {
                      handleToggleModel(
                        model,
                        (e.currentTarget as HTMLInputElement).checked
                      );
                    }}
                    class="w-4 h-4 text-primary bg-base-100 border-base-300 rounded focus:ring-primary focus:ring-2"
                  />
                </div>

                <div class="flex items-center justify-center w-16">
                  <button
                    onclick={() => {
                      if (currentProvider && currentProvider.id) {
                        providerActions.toggleModelFavorite(
                          currentProvider.id,
                          model.id,
                          !model.favorite
                        );
                      }
                    }}
                    class="p-1 rounded hover:bg-base-300 transition-colors"
                    aria-label={model.favorite
                      ? "Remove from favorites"
                      : "Add to favorites"}
                  >
                    <Star
                      size={16}
                      class={model.favorite
                        ? "text-base-content fill-current"
                        : "text-base-content/60 hover:text-error"}
                    />
                  </button>
                </div>

                <div class="flex items-center justify-center w-14">
                  <IconButton
                    icon={Info}
                    iconSize={16}
                    size="w-6 h-6"
                    ariaLabel="查看模型信息"
                    onclick={() => openModelInfo(model)}
                  />
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="text-center text-sm py-8 text-base-content/70">
          {#if isCustom}
            该自定义供应商暂无模型，点击「添加模型」手动添加端点支持的 model id
          {:else}
            暂无模型数据，请检查供应商配置或网络连接
          {/if}
        </div>
      {/if}
    {/if}
  </main>
</div>

<ModelInfoModal
  open={showModelInfo}
  model={selectedModel}
  onClose={closeModelInfo}
/>

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

<!-- 禁用供应商确认弹窗 -->
<ConfirmModal
  open={showDisableConfirm}
  title="关闭供应商"
  message={relatedChatsCount > 0
    ? `检测到有 <span class='font-medium'>${relatedChatsCount}</span> 个会话正在使用 <span class='font-medium'>${currentProvider?.name}</span>。<br/><br/>关闭此供应商后，这些会话将无法使用该供应商的模型。<br/><br/>确定要关闭吗？`
    : `确认关闭 <span class='font-medium'>${currentProvider?.name}</span> 吗？`}
  confirmText="关闭"
  cancelText="取消"
  confirmButtonStyle="danger"
  onClose={() => (showDisableConfirm = false)}
  onConfirm={confirmDisableProvider}
  onCancel={cancelDisableProvider}
/>

<!-- 禁用模型确认弹窗 -->
<ConfirmModal
  open={showModelDisableConfirm}
  title="禁用模型"
  message={relatedChatsCount > 0
    ? `检测到有 <span class='font-medium'>${relatedChatsCount}</span> 个会话正在使用模型 <span class='font-medium'>${modelToDisable?.name}</span>。<br/><br/>禁用此模型后，这些会话将无法使用该模型。<br/><br/>确定要禁用吗？`
    : `确认禁用模型 <span class='font-medium'>${modelToDisable?.name}</span> 吗？`}
  confirmText="禁用"
  cancelText="取消"
  confirmButtonStyle="danger"
  onClose={() => (showModelDisableConfirm = false)}
  onConfirm={confirmDisableModel}
  onCancel={cancelDisableModel}
/>
