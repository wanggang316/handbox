<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { providerState, providerActions, getProviderIcon, getPreProvider } from "$lib/states/provider.svelte";
  import type { Provider, ProviderConfig } from "$lib/types/provider";
  import {
    RotateCw,
    Trash2,
    ChevronLeft,
    Edit,
    RefreshCw,
    Settings2,
    Save,
    Magnet,
    ListChecks,
    Activity,
  } from "@lucide/svelte";
  import ModelSelectModal from "$lib/components/settings/ModelSelectModal.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import TextRow from "$lib/components/ui/table/TextRow.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  let providerId = $state("");
  let showModelsModal = $state(false);
  let showDeleteConfirm = $state(false);
  let isProbing = $state(false);
  let isSaving = $state(false);
  let isLoadingModels = $state(false);

  // 当前供应商
  let currentProvider = $state<Provider | null>(null);

  // 配置表单
  let formData = $state({
    name: "",
    base_url: "",
    api_key: "",
    enabled: false,
  });

  // 获取预定义供应商信息  
  let preProvider = $derived(currentProvider ? getPreProvider(currentProvider) : null);
  let providerIcon = $derived(currentProvider ? getProviderIcon(currentProvider) : null);

  onMount(() => {
    providerId = $page.params.id || "";
    loadProvider();
  });

  async function loadProvider() {
    if (!providerId) return;
    
    try {
      // 从全局状态中查找供应商
      const provider = providerState.providers.find(p => p.id === providerId);
      
      if (provider) {
        currentProvider = provider;
        // 填充表单数据
        formData = {
          name: provider.name,
          base_url: provider.base_url,
          api_key: provider.api_key,
          enabled: provider.enabled,
        };
      } else {
        // 如果本地没有，先加载供应商列表
        await providerActions.loadProviders();
        const loadedProvider = providerState.providers.find(p => p.id === providerId);
        
        if (loadedProvider) {
          currentProvider = loadedProvider;
          formData = {
            name: loadedProvider.name,
            base_url: loadedProvider.base_url,
            api_key: "",
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

  async function handleProbe() {
    if (!currentProvider) return;
    
    isProbing = true;
    try {
      const result = await providerActions.probeProvider(currentProvider.id);
      console.log("Probe result:", result);
    } catch (error) {
      console.error("Probe failed:", error);
    } finally {
      isProbing = false;
    }
  }

  async function handleSave() {
    if (!currentProvider) return;
    
    isSaving = true;
    try {
      // 准备更新配置
      const config: Partial<ProviderConfig> = {
        name: formData.name,
        provider_type: currentProvider.provider_type,
        base_url: formData.base_url,
        enabled: formData.enabled,
      };
      
      // 如果 API key 有值，也更新它
      if (formData.api_key.trim()) {
        config.api_key = formData.api_key;
      }
      console.log("currentProvider:", currentProvider.id, "config:", config);
      await providerActions.updateProvider(currentProvider.id, config);
      console.log("Provider config saved successfully");
      
      // 更新本地当前供应商数据
      // 更新本地当前供应商数据
      const currentId = currentProvider?.id;
      if (currentId) {
        const updatedProvider = providerState.providers.find(p => p.id === currentId);
        if (updatedProvider) {
          currentProvider = updatedProvider;
        }
      }
    } catch (error) {
      console.error("Save failed:", error);
    } finally {
      isSaving = false;
    }
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

  function handleModelsConfirm(
    event: CustomEvent<{ selectedModels: string[] }>,
  ) {
    const { selectedModels } = event.detail;
    console.log("Selected models:", selectedModels);
    // showModelsModal = false; // 让 Modal 组件自己处理关闭动画
  }

  function handleCloseModels() {
    showModelsModal = false; // 这里可以保留，因为是 Modal 组件调用的
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
      showDeleteConfirm = false;
    }
  }

  function handleBack() {
    goto("/settings/models");
  }

  function handleRefreshModels(e: CustomEvent<any>): void {
    showModelsModal = true
  }

  function handleTestKey(): void {
    // 测试 API Key 的功能，可以调用 handleProbe
    handleProbe();
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
    <div class="w-6 h-6 bg-gray-300 rounded flex items-center justify-center text-xs font-bold">
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
    <TableBaseRow label={currentProvider.name} icon={iconSnippet}>
      <div class="flex flex-row items-center gap-4">
        <IconButton
          icon={Edit}
          on:click={() => {}}
          disabled={true}
        />

        <IconButton
          icon={Trash2}
          on:click={handleDelete}
        />

        <Toggle 
          checked={formData.enabled} 
          on:change={(e) => formData.enabled = e.detail}
        />
      </div>
    </TableBaseRow>
    {/if}

    <div class="flex items-center justify-end">
      <Button 
        on:click={handleProbe} 
        variant="clear" 
        size="sm"
        disabled={isProbing || !currentProvider}
      >
        <Activity size={14} class={isProbing ? "animate-spin" : ""} />
        {isProbing ? "检测中" : "检测"}
      </Button>

      <Button 
        on:click={handleSave} 
        variant="clear" 
        size="sm"
        disabled={isSaving || !currentProvider}
      >
        <Save size={14} />
        {isSaving ? "保存中" : "保存"}
      </Button>
    </div>

    <TableGroup>
      <TextRow
        layout="vertical"
        isPassword
        label="API Key"
        bind:value={formData.api_key}
      />
      <TextRow 
        layout="vertical" 
        label="Base URL" 
        bind:value={formData.base_url} 
      />
    </TableGroup>

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
      {@const providerModels = providerState.availableModels.filter(m => m.provider_id === currentProvider?.id)}
      {#if providerModels.length > 0}
        <TableGroup title={currentProvider.name}>
          {#each providerModels as model}
            <TableBaseRow label={model.name} py="2">
              <div class="flex flex-row items-center gap-2">
                <Toggle 
                  checked={model.enabled} 
                  on:change={(e) => {
                    if (currentProvider) {
                      providerActions.toggleModel(currentProvider.id, model.id, e.detail);
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

<!-- 模型选择弹窗 -->
<ModelSelectModal
  open={showModelsModal}
  {providerId}
  on:close={handleCloseModels}
  on:confirm={handleModelsConfirm}
/>

<!-- 删除确认弹窗 -->
{#if showDeleteConfirm}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
  >
    <div class="bg-white rounded-2xl p-6 max-w-md mx-4">
      <h3 class="text-lg font-semibold text-slate-900 mb-2">确认删除</h3>
      <p class="text-slate-600 mb-6">
        确定要删除该供应商吗？此操作不可撤销，所有相关配置和数据都将被清除。
      </p>

      <div class="flex items-center justify-end gap-3">
        <button
          onclick={() => (showDeleteConfirm = false)}
          class="px-4 py-2 text-slate-600 bg-slate-100 rounded-lg hover:bg-slate-200 transition-colors"
        >
          取消
        </button>
        <button
          onclick={confirmDelete}
          disabled={providerState.isLoading}
          class="flex items-center gap-2 px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {#if providerState.isLoading}
            <RotateCw class="w-4 h-4 animate-spin" />
            <span>删除中...</span>
          {:else}
            <Trash2 class="w-4 h-4" />
            <span>确认删除</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
