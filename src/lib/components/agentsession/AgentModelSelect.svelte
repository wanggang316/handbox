<script lang="ts">
  import { ChevronsUpDown, Check } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import {
    providerState,
    providerActions,
    getProviderIconById,
  } from "$lib/states/provider.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";

  // 会话级模型选择器：向上弹出、按 provider 分组的模型列表。选中即回调一个完整的
  // ModelWithProvider（含 provider_id），调用方据此把 modelId + providerId **成对**
  // 写入会话——杜绝「有 model 无 provider」的半配置态（Header 幻影 / 发送被拦的根因）。
  // 复用于 AgentInput 组合框与 quick-action 浮层。
  interface Props {
    // 当前选中的模型（从会话 modelId/providerId 反查；解析不到为 null → 显示占位）。
    selected: ModelWithProvider | null;
    onSelect: (model: ModelWithProvider) => void;
    // 触发按钮尺寸（组合框用 h-7，与相邻图标按钮对齐）。
    size?: string;
  }
  let { selected, onSelect, size = "h-7" }: Props = $props();

  let open = $state(false);

  // 打开时 lazy-load provider catalog（命中即返回）；空目录才拉取。
  $effect(() => {
    if (open && providerState.providersWithModels.length === 0) {
      providerActions
        .loadProvidersWithModels()
        .catch((e) => console.error("Failed to load models:", e));
    }
  });

  // 点击外部关闭：触发按钮经 stopPropagation 不冒泡到 window。
  $effect(() => {
    if (!open) return;
    const handler = () => (open = false);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    open = !open;
  }

  function pick(model: ModelWithProvider) {
    onSelect(model);
    open = false;
  }

  const selectedIcon = $derived(
    selected ? getProviderIconById(selected.provider_id) : undefined,
  );

  // provider 分组：直接消费 providersWithModels（每组一个 provider + 其 models），
  // 逐 model 构造 ModelWithProvider 供选中回调。只列有模型的 provider。
  const groups = $derived(
    providerState.providersWithModels
      .filter((p) => p.models.length > 0)
      .map((p) => ({
        id: p.id ?? p.name,
        name: p.name,
        icon: p.id ? getProviderIconById(p.id) : undefined,
        models: p.models.map(
          (m): ModelWithProvider => ({
            ...m,
            providerName: p.name,
            providerType: p.provider_type,
          }),
        ),
      })),
  );
</script>

<div class="relative">
  <button
    type="button"
    class={`flex ${size} items-center gap-1.5 rounded-md pl-1.5 pr-2 transition-colors ${
      open
        ? "bg-base-300 text-base-content"
        : "text-base-content hover:bg-base-300"
    }`}
    aria-label={t("agent.input.selectModel")}
    aria-haspopup="menu"
    aria-expanded={open}
    title={selected?.name ?? t("agent.input.selectModel")}
    onclick={toggle}
  >
    {#if selected}
      {#if selectedIcon}
        <img
          src={selectedIcon}
          alt={selected.providerName}
          class="h-4 w-4 shrink-0 rounded object-contain"
        />
      {/if}
      <span class="max-w-[160px] truncate text-sm">{selected.name}</span>
    {:else}
      <span class="max-w-[160px] truncate text-sm text-warning"
        >{t("agent.input.selectModel")}</span
      >
    {/if}
    <ChevronsUpDown size={13} class="shrink-0 opacity-60" />
  </button>

  {#if open}
    <!-- 向上展开（bottom-full）：组合框在底部，列表浮于按钮上方以免落屏外。
         stopPropagation 防止菜单内点击冒泡到 window 触发外部关闭。 -->
    <div
      class="absolute bottom-full left-0 z-40 mb-2 max-h-80 w-72 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
      role="menu"
      tabindex="-1"
      onclick={(event) => event.stopPropagation()}
      onkeydown={() => {}}
    >
      {#if groups.length === 0}
        <div class="px-2 py-1.5 text-xs text-base-content/50">
          {t("agent.input.noAvailableModels")}
        </div>
      {:else}
        {#each groups as group (group.id)}
          <div
            class="flex items-center gap-1.5 px-2 pb-1 pt-2 text-[11px] font-medium uppercase tracking-wider text-base-content/40"
          >
            {#if group.icon}
              <img
                src={group.icon}
                alt={group.name}
                class="h-3.5 w-3.5 shrink-0 rounded object-contain"
              />
            {/if}
            <span class="truncate">{group.name}</span>
          </div>
          {#each group.models as model (model.id)}
            {@const active =
              selected?.id === model.id &&
              selected?.provider_id === model.provider_id}
            <button
              type="button"
              role="menuitem"
              class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                active ? "bg-base-300/60" : ""
              }`}
              onclick={() => pick(model)}
            >
              <span class="min-w-0 flex-1 truncate text-sm text-base-content">
                {model.name}
              </span>
              {#if active}
                <Check size={14} class="shrink-0 text-primary" />
              {/if}
            </button>
          {/each}
        {/each}
      {/if}
    </div>
  {/if}
</div>
