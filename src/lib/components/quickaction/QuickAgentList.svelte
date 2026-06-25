<!--
  Quick Action 浮层「选择步」的 Agent 列表。

  纯呈现:渲染父级已过滤好的 Agent 列表,高亮 `highlightIndex` 指向的项;键盘导航
  (↑↓ / ↵)由父级经 QuickInput 的语义化回调驱动,本组件只负责:
  - 点击某项 → onSelect(agent)
  - 鼠标移到某项 → onHover(index)(让键盘高亮跟随鼠标,避免两套高亮打架)
  - 高亮项变化时自动滚入可视区(键盘导航越过视口时)
-->
<script lang="ts">
  import { Bot } from "@lucide/svelte";
  import type { Agent } from "$lib/types";

  interface Props {
    agents: Agent[];
    highlightIndex: number;
    onSelect?: (agent: Agent) => void;
    onHover?: (index: number) => void;
  }

  let { agents, highlightIndex, onSelect = () => {}, onHover = () => {} }: Props =
    $props();

  let rowRefs = $state<(HTMLButtonElement | null)[]>([]);

  // 键盘高亮越过视口时,把高亮项滚进可视区。
  $effect(() => {
    const el = rowRefs[highlightIndex];
    el?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="flex flex-col gap-0.5 p-2">
  {#each agents as agent, index (agent.id)}
    <button
      bind:this={rowRefs[index]}
      type="button"
      class="qa-row"
      class:is-active={index === highlightIndex}
      onclick={() => onSelect(agent)}
      onmousemove={() => onHover(index)}
    >
      <span class="qa-row-icon">
        <Bot size={15} class="text-[var(--base-content)]/70" />
      </span>
      <span class="min-w-0 flex-1 truncate text-left text-[14px]">{agent.name}</span>
      {#if agent.model}
        <span class="shrink-0 truncate text-[11px] text-[var(--base-content)]/45">
          {agent.model}
        </span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .qa-row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    border-radius: 8px;
    padding: 0.5rem 0.625rem;
    color: var(--base-content);
    transition: background-color 0.1s ease;
  }
  .qa-row.is-active {
    background: color-mix(in srgb, var(--base-content) 9%, transparent);
  }
  .qa-row-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    flex-shrink: 0;
    border-radius: 6px;
    background: color-mix(in srgb, var(--base-content) 7%, transparent);
  }
</style>
