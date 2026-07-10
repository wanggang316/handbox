<script lang="ts">
  import InfoTooltip from "../InfoTooltip.svelte";

  interface Props {
    label?: string;
    description?: string; // 左侧 label 下的副标题（Linear 式设置行）
    icon?: any; // Lucide 图标组件
    layout?: "horizontal" | "vertical";
    py?: string;
    rightContent?: any; // 标题行右边的内容
    helpText?: string; // 可选的帮助提示文本，显示为问号图标
    error?: string; // 可选的字段级错误，行内显示在控件下方
    children?: any;
  }

  let {
    label,
    description,
    icon,
    layout = "horizontal",
    py = "4",
    rightContent,
    helpText,
    error,
    children,
  }: Props = $props();

  const errorId = `tblrow-${Math.random().toString(36).slice(2)}-error`;
</script>

{#snippet labelHeader()}
  <div class="flex min-w-0 flex-col gap-0.5">
    <div class="flex items-center gap-2">
      {#if icon}
        {@render icon({ class: "w-4 h-4 text-base-content/70" })}
      {/if}
      <div class="text-sm text-base-content">{label}</div>
      {#if helpText}
        <InfoTooltip content={helpText} />
      {/if}
    </div>
    {#if description}
      <div class="text-[13px] leading-snug text-base-content/55">
        {description}
      </div>
    {/if}
  </div>
{/snippet}

<div class="px-6 py-{py}">
  {#if label}
    {#if layout === "vertical"}
      <div class="space-y-2">
        <div class="flex items-center justify-between gap-4">
          {@render labelHeader()}
          {#if rightContent}
            <div>
              {@render rightContent?.()}
            </div>
          {/if}
        </div>
        <div>
          {@render children?.()}
        </div>
        {#if error}
          <p id={errorId} class="text-xs text-error mt-1">{error}</p>
        {/if}
      </div>
    {:else}
      <div class="flex items-center justify-between gap-4">
        {@render labelHeader()}
        {#if error}
          <div class="flex flex-col items-end">
            <div class="flex justify-end">
              {@render children?.()}
            </div>
            <p id={errorId} class="text-xs text-error mt-1">{error}</p>
          </div>
        {:else}
          <div class="flex justify-end">
            {@render children?.()}
          </div>
        {/if}
      </div>
    {/if}
  {:else}
    {@render children?.()}
    {#if error}
      <p id={errorId} class="text-xs text-error mt-1">{error}</p>
    {/if}
  {/if}
</div>
