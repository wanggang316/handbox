<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import { Search, Star, Check } from "@lucide/svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import { providerState, providerActions } from "$lib/states/provider.svelte";
  import { onMount } from "svelte";

  interface Props {
    open?: boolean;
    onClose?: () => void;
    selectedModel?: ModelWithProvider | null;
    onModelSelect?: (model: ModelWithProvider) => void;
  }

  let {
    open = $bindable(false),
    onClose = () => {},
    selectedModel = null,
    onModelSelect = () => {},
  }: Props = $props();

  let searchQuery = $state("");
  let showFavoritesOnly = $state(false);

  // 从状态管理中获取数据 (直接使用 provider 状态，避免透传)
  const allModels = $derived(() => {
    return providerState.providersWithModels.flatMap((provider) =>
      provider.models.map((model) => ({
        ...model,
        providerName: provider.name,
        providerType: provider.provider_type,
      }))
    );
  });

  const favoriteModels = $derived(() => {
    return allModels().filter((model) => model.favorite);
  });
  const selectedModelId = $derived(selectedModel?.id || "");

  // 过滤后的模型
  const filteredModels = $derived(() => {
    let models = showFavoritesOnly ? favoriteModels() : allModels();

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      models = models.filter(
        (model: ModelWithProvider) =>
          model.name.toLowerCase().includes(query) ||
          (model.providerName &&
            model.providerName.toLowerCase().includes(query))
      );
    }

    return models;
  });

  // 按供应商分组
  const groupedModels = $derived(() => {
    const groups: Record<string, ModelWithProvider[]> = {};

    filteredModels().forEach((model: ModelWithProvider) => {
      const key = model.providerName || "Unknown";
      if (!groups[key]) {
        groups[key] = [];
      }
      groups[key].push(model);
    });

    return groups;
  });

  function handleModelSelect(model: ModelWithProvider) {
    onModelSelect(model);
    handleClose();
  }

  async function handleToggleFavorite(model: ModelWithProvider) {
    try {
      // 直接使用 providerActions，避免 chatState 透传
      await providerActions.toggleModelFavorite(
        model.provider_id,
        model.id,
        !model.favorite
      );
    } catch (error) {
      console.error("Failed to toggle favorite:", error);
    }
  }

  // 组件挂载时初始化数据
  onMount(async () => {
    if (providerState.providersWithModels.length === 0) {
      await providerActions.loadProvidersWithModels();
    }
  });

  function handleClose() {
    open = false;
    onClose();
  }

  function clearSearch() {
    searchQuery = "";
  }
</script>

<Modal
  bind:open
  onClose={handleClose}
  showCloseButton={false}
  closeOnBackdropClick={true}
>
  <div class="w-[600px] h-[70vh] max-h-[70vh] flex flex-col">
    <!-- 搜索和过滤器区域 -->
    <div class="px-6 py-4 border-b border-base-300 space-y-3">
      <!-- 搜索框 -->
      <div class="relative">
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/80"
          size={16}
        />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="搜索模型..."
          class="w-full pl-10 pr-4 py-2 border border-base-300 rounded-xl focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary"
        />
        {#if searchQuery}
          <button
            onclick={clearSearch}
            class="absolute right-3 top-1/2 -translate-y-1/2 text-base-content/60 hover:text-base-content"
          >
            ×
          </button>
        {/if}
      </div>

      <!-- 过滤器按钮 -->
      <div class="flex items-center justify-between gap-3">
        <div class="text-xs text-base-content/70">
          共找到 {filteredModels().length} 个模型
        </div>

        <button
          onclick={() => (showFavoritesOnly = !showFavoritesOnly)}
          class="flex items-center gap-1 px-2 py-1 rounded-md text-sm {showFavoritesOnly
            ? 'bg-warning/10 text-warning border border-warning/30'
            : 'bg-base-200 text-base-content/80 border border-base-200 hover:bg-base-300'}"
        >
          <Star
            size={14}
            class={showFavoritesOnly
              ? "fill-warning text-warning"
              : "text-base-content/60"}
          />
          收藏模型
        </button>
      </div>
    </div>

    <!-- 模型列表 -->
    <div class="flex-1 overflow-y-auto">
      {#if filteredModels().length === 0}
        <div
          class="flex flex-col items-center justify-center py-12 text-base-content/70"
        >
          <Search size={48} class="mb-4 opacity-50" />
          <p class="text-lg">未找到匹配的模型</p>
          <p class="text-sm">尝试调整搜索条件或清除过滤器</p>
        </div>
      {:else}
        <!-- 分组模型列表 -->
        <div class="px-6 py-4 space-y-6">
          {#each Object.entries(groupedModels()) as [providerName, models]}
            <div>
              <h3 class="text-sm font-medium text-base-content mb-3">
                {providerName}
              </h3>
              <div class="space-y-1">
                {#each models as model, index (model.id)}
                  <div
                    class="w-full flex flex-row items-center gap-4 px-4 py-1 {index %
                      2 ===
                    0
                      ? 'bg-base-100'
                      : 'bg-base-200'} hover:bg-base-300"
                  >
                    <button
                      onclick={() => handleModelSelect(model)}
                      class="flex-1 text-left"
                    >
                      <div class="flex items-center gap-2">
                        <span class="text-base-content text-sm"
                          >{model.name}</span
                        >
                        {#if model.id === selectedModelId}
                          <Check size={16} class="text-primary" />
                        {/if}
                      </div>
                    </button>
                    <button
                      onclick={(e) => {
                        e.stopPropagation();
                        handleToggleFavorite(model);
                      }}
                      class="p-1 hover:bg-base-200 rounded transition-colors ml-2"
                    >
                      <Star
                        size={16}
                        class={model.favorite
                          ? "fill-warning text-warning"
                          : "text-base-content/60 hover:text-base-content"}
                      />
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</Modal>
