<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import {
    Search,
    Star,
    Check,
    ChevronDown,
  } from "@lucide/svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import {
    providerState,
    providerActions,
    getProviderIconById,
  } from "$lib/states/provider.svelte";

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
  let selectedProviderFilter = $state<string>("all");
  let hoveredModel = $state<ModelWithProvider | null>(null);
  let tooltipPosition = $state({ x: 0, y: 0 });
  // 从 provider 状态派生可用模型，仅关注已启用的供应商与模型
  const allModels = $derived(() => {
    return providerState.providersWithModels
      .filter((provider) => provider.enabled)
      .flatMap((provider) =>
        provider.models
          .filter((model) => model.enabled)
          .map((model) => ({
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
  const isLoadingModels = $derived(() => providerState.isLoadingWithModels);

  // 获取所有可用的供应商列表（用于筛选下拉框）
  const availableProviders = $derived(() => {
    const providers = new Set<string>();
    allModels().forEach((model) => {
      if (model.providerName) {
        providers.add(model.providerName);
      }
    });
    return Array.from(providers).sort();
  });

  // 过滤后的模型
  const filteredModels = $derived(() => {
    let models = showFavoritesOnly ? favoriteModels() : allModels();

    // 按供应商筛选
    if (selectedProviderFilter !== "all") {
      models = models.filter((model: ModelWithProvider) => {
        return model.providerName === selectedProviderFilter;
      });
    }

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

  // 当 Modal 打开时检查是否需要刷新数据
  $effect(() => {
    if (!open || providerState.isLoadingWithModels) {
      return;
    }

    // 如果需要刷新或者数据为空，则加载数据
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      console.log("ChatModelSelectModal: Loading providers with models");
      providerActions.loadProvidersWithModels();
    }
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
        !model.favorite,
        { skipRefreshFlag: true }
      );
    } catch (error) {
      console.error("Failed to toggle favorite:", error);
    }
  }

  function handleClose() {
    open = false;
    onClose();
  }

  function clearSearch() {
    searchQuery = "";
  }

  function handleMouseEnter(model: ModelWithProvider, event: MouseEvent) {
    hoveredModel = model;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    tooltipPosition = {
      x: rect.right + 10,
      y: rect.top,
    };
  }

  function handleMouseLeave() {
    hoveredModel = null;
  }
</script>

<Modal
  bind:open
  onClose={handleClose}
  showCloseButton={false}
  closeOnBackdropClick={true}
>
  <div class="w-[500px] h-[70vh] max-h-[70vh] flex flex-col">
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
          {#if isLoadingModels()}
            正在加载模型...
          {:else}
            共找到 {filteredModels().length} 个模型
          {/if}
        </div>

        <div class="flex items-center gap-2">
          <!-- 供应商筛选 -->
          <Select
            bind:value={selectedProviderFilter}
            options={[
              { value: "all", label: "全部供应商" },
              ...availableProviders().map((p) => ({ value: p, label: p })),
            ]}
            autoWidth={true}
            size="sm"
          />

          <!-- 收藏筛选 -->
          <button
            onclick={() => (showFavoritesOnly = !showFavoritesOnly)}
            class="flex items-center gap-1 px-2 py-1 rounded-md text-xs {showFavoritesOnly
              ? 'bg-warning/10 text-warning border border-warning/30'
              : 'bg-base-200 text-base-content border border-base-200 hover:bg-base-300'}"
          >
            <Star
              size={14}
              class={showFavoritesOnly
                ? "fill-warning text-warning"
                : "text-base-content/60"}
            />
            收藏
          </button>
        </div>
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
        <div class="px-4 py-4 space-y-2">
          {#each Object.entries(groupedModels()) as [providerName, models]}
            <TableGroup
              title={providerName}
              collapsible={true}
              defaultCollapsed={false}
              showDivider={false}
            >
              {#each models as model, index (model.id)}
                <div
                  role="button"
                  tabindex="-1"
                  class="flex flex-row px-4 py-1 {index % 2 === 0
                    ? 'bg-base-100'
                    : 'bg-base-200'} hover:bg-base-300"
                  onmouseenter={(e) => handleMouseEnter(model, e)}
                  onmouseleave={handleMouseLeave}
                >
                  <button
                    onclick={() => handleModelSelect(model)}
                    class="flex-1 flex items-center justify-between gap-2 text-left"
                  >
                    <div class="flex items-center gap-2">
                      <span class="text-base-content text-sm">{model.name}</span
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
                    class="p-1 hover:bg-base-200 rounded transition-colors"
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
            </TableGroup>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Tooltip for model details -->
  {#if hoveredModel}
    {@const providerIcon = getProviderIconById(hoveredModel.provider_id)}
    <div
      class="fixed z-[9999] bg-base-100 border border-base-300 rounded-lg shadow-xl px-4 pt-4 pb-0 min-w-[280px] max-w-[380px]"
      style="left: {tooltipPosition.x}px; top: {tooltipPosition.y}px;"
    >
      <div class="space-y-1">
        <!-- 模型名称 - 大字体，带供应商图标 -->
        <div class="flex items-center gap-2">
          {#if providerIcon}
            <img
              src={providerIcon}
              alt={hoveredModel.providerName}
              class="h-3.5 w-3.5 rounded-md object-contain flex-shrink-0"
            />
          {/if}
          <div class="text-base text-base-content">
            {hoveredModel.name}
          </div>
        </div>

        <!-- 模型 ID - tag 样式 -->
        <div class="flex mb-2">
          <span
            class="inline-block px-2 py-1 text-xs bg-base-200 text-base-content/70 rounded-lg break-all"
          >
            {hoveredModel.id}
          </span>
        </div>

        <!-- 其他信息 -->
        <div class="space-y-2 mb-4 text-xs">
          {#if hoveredModel.context_length}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">上下文长度</span>
              <span class="font-semibold text-base-content">
                {hoveredModel.context_length.toLocaleString()} tokens
              </span>
            </div>
          {/if}
          {#if hoveredModel.output_max_tokens}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">最大输出长度</span>
              <span class="font-semibold text-base-content">
                {hoveredModel.output_max_tokens.toLocaleString()} tokens
              </span>
            </div>
          {/if}
          {#if hoveredModel.pricing?.input_text}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">输入价格</span>
              <span class="font-semibold text-base-content">
                {hoveredModel.pricing.input_text}
              </span>
            </div>
          {/if}
          {#if hoveredModel.pricing?.output_text}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">输出价格</span>
              <span class="font-semibold text-base-content">
                {hoveredModel.pricing.output_text}
              </span>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</Modal>
