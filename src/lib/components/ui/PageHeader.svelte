<script lang="ts">
  import type { Snippet } from "svelte";

  // 页面级统一 Header：Agents / Jobs / 设置各页共用同一套标题排版。
  interface Props {
    title: string;
    /** 标题旁的补充信息（如数量） */
    meta?: string;
    description?: string;
    /** 右侧操作区（按钮等） */
    actions?: Snippet;
    /** 标题区下方扩展行（搜索框等） */
    children?: Snippet;
  }

  let { title, meta = "", description = "", actions, children }: Props =
    $props();
</script>

<header class="flex flex-col gap-3">
  <div class="flex items-center justify-between gap-4">
    <div class="flex min-w-0 items-baseline gap-2.5">
      <h1 class="truncate text-xl font-semibold text-base-content">{title}</h1>
      {#if meta}
        <span class="shrink-0 text-sm text-base-content/50">{meta}</span>
      {/if}
    </div>
    {#if actions}
      <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
    {/if}
  </div>
  {#if description}
    <p class="text-sm text-base-content/60">{description}</p>
  {/if}
  {#if children}
    {@render children()}
  {/if}
</header>
