<!--
  Agent list for the Quick Action overlay's selection step. Purely presentational:
  the parent filters agents and drives keyboard navigation via QuickInput's
  callbacks; this only emits onSelect (click) and onHover (mouse moves the
  keyboard highlight, so there's one highlight rather than two competing ones).
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

  // Keep the highlighted item visible when keyboard navigation moves past the viewport.
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
    transition: background-color var(--dur-fast) ease;
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
