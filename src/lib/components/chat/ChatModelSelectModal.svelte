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
    Eye as EyeIcon,
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
  const allModels = $derived(
    providerState.providersWithModels
      .filter((provider) => provider.enabled)
      .flatMap((provider) =>
        provider.models
          .filter((model) => model.enabled)
          .map((model) => ({
            ...model,
            providerName: provider.name,
            providerType: provider.provider_type,
          }))
      )
  );

  const favoriteModels = $derived(allModels.filter((model) => model.favorite));
  const selectedModelId = $derived(selectedModel?.id || "");
  const isLoadingModels = $derived(providerState.isLoadingWithModels);

  // 获取所有可用的供应商列表（用于筛选下拉框）
  let availableProvidersResult: string[] = $state([]);

  $effect(() => {
    const providers = new Set<string>();
    for (const model of allModels) {
      if (model.providerName) {
        providers.add(model.providerName);
      }
    }
    availableProvidersResult = Array.from(providers).sort();
  });

  // 过滤后的模型
  let filteredModelsResult: ModelWithProvider[] = $state([]);

  $effect(() => {
    let models = showFavoritesOnly ? favoriteModels : allModels;

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

    filteredModelsResult = models;
  });

  // 按供应商分组
  let groupedModelsResult: Record<string, ModelWithProvider[]> = $state({});

  $effect(() => {
    const groups: Record<string, ModelWithProvider[]> = {};

    for (const model of filteredModelsResult) {
      const key = model.providerName || "Unknown";
      if (!groups[key]) {
        groups[key] = [];
      }
      groups[key].push(model);
    }

    groupedModelsResult = groups;
  });

  // 当 Modal 打开时检查是否需要刷新数据
  $effect(() => {
    if (!open) {
      return;
    }

    // 移除了对 isLoadingWithModels 的检查
    // 这样即使正在加载，Modal 也会立即显示
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      console.log("ChatModelSelectModal: Loading providers with models");
      // 使用 .catch() 避免未处理的 Promise rejection
      providerActions.loadProvidersWithModels().catch((err) => {
        console.error("Failed to load models:", err);
      });
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
          class="w-full pl-10 pr-4 py-2 border border-[var(--hairline)] bg-base-300 rounded-md focus:border-primary"
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
          {#if isLoadingModels}
            正在加载模型...
          {:else}
            共找到 {filteredModelsResult.length} 个模型
          {/if}
        </div>

        <div class="flex items-center gap-2">
          <!-- 供应商筛选 -->
          <Select
            bind:value={selectedProviderFilter}
            options={[
              { value: "all", label: "全部供应商" },
              ...availableProvidersResult.map((p) => ({ value: p, label: p })),
            ]}
            autoWidth={true}
            size="sm"
          />

          <!-- 收藏筛选 -->
          <button
            onclick={() => (showFavoritesOnly = !showFavoritesOnly)}
            class="flex items-center gap-1 px-2 py-1 rounded-md text-xs {showFavoritesOnly
              ? 'bg-warning/10 text-warning border border-warning/30'
              : 'bg-base-300 text-base-content border border-[var(--hairline)] hover:bg-base-300/80'}"
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
      {#if filteredModelsResult.length === 0}
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
          {#each Object.entries(groupedModelsResult) as [providerName, models]}
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
                  class="flex flex-row px-4 py-1 hover:bg-base-300 transition-colors"
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
                      {#if model.support_image}
                        <EyeIcon
                          size={14}
                          class="text-info"
                          aria-label="支持图片生成"
                        />
                      {/if}
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
                    class="p-1 hover:bg-base-300 rounded transition-colors"
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
      class="fixed z-[9999] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-4 pt-4 pb-0 min-w-[280px] max-w-[380px]"
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
          <div class="text-base text-base-content flex items-center gap-2">
            {hoveredModel.name}
            {#if hoveredModel.support_image}
              <EyeIcon size={14} class="text-info" aria-label="支持图片生成" />
            {/if}
          </div>
        </div>

        <!-- 模型 ID - tag 样式 -->
        <div class="flex mb-2">
          <span
            class="inline-block px-2 py-1 text-xs bg-base-300 text-base-content/70 rounded-md break-all"
          >
            {hoveredModel.id}
          </span>
        </div>

        <!-- 其他信息 -->
        <div class="space-y-2 mb-4 text-xs">
          {#if hoveredModel.display_context_length}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">上下文长度</span>
              <span class="font-medium text-base-content">
                {hoveredModel.display_context_length}
              </span>
            </div>
          {/if}
          {#if hoveredModel.display_output_max_tokens}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">最大输出长度</span>
              <span class="font-medium text-base-content">
                {hoveredModel.display_output_max_tokens}
              </span>
            </div>
          {/if}
          {#if hoveredModel.pricing?.input_text}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">输入价格</span>
              <span class="font-medium text-base-content">
                {hoveredModel.pricing.input_text}
              </span>
            </div>
          {/if}
          {#if hoveredModel.pricing?.output_text}
            <div class="flex justify-between items-center">
              <span class="text-base-content/70">输出价格</span>
              <span class="font-medium text-base-content">
                {hoveredModel.pricing.output_text}
              </span>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</Modal>
