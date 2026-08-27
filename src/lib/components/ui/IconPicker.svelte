<script lang="ts">
  /**
   * Lucide icon picker over the curated {@link AGENT_ICONS} set.
   *
   * The current icon is the trigger; the grid opens beneath it. Picking
   * replaces and closes; picking the icon that is already selected CLEARS it
   * (back to the default), which is how a chosen icon is undone without a
   * separate control. An outside click closes without changing anything.
   *
   * `value` is the persisted kebab-case Lucide name, or "" for the default.
   */
  import { AGENT_ICONS, resolveAgentIcon } from "$lib/utils/agentIcons";

  interface Props {
    value: string;
    onSelect: (name: string) => void;
    /** Accessible name for the trigger; also its hover tooltip. */
    label: string;
    disabled?: boolean;
  }

  let { value, onSelect, label, disabled = false }: Props = $props();

  let open = $state(false);

  const CurrentIcon = $derived(resolveAgentIcon(value));

  // Scoped to this instance's wrapper, so two pickers on one page do not close
  // each other and a click inside the grid does not close it.
  let wrapper = $state<HTMLElement | null>(null);

  function handleOutside(event: MouseEvent) {
    if (!open) return;
    if (wrapper && !wrapper.contains(event.target as Node)) {
      open = false;
    }
  }

  function pick(name: string) {
    onSelect(value === name ? "" : name);
    open = false;
  }
</script>

<div bind:this={wrapper} class="relative flex-shrink-0">
  <button
    type="button"
    aria-expanded={open}
    aria-label={label}
    title={label}
    {disabled}
    class="flex h-10 w-10 items-center justify-center rounded-lg bg-base-200 text-base-content/70 transition-colors enabled:hover:bg-base-300 enabled:hover:text-base-content disabled:opacity-60"
    onclick={() => (open = !open)}
  >
    <CurrentIcon size={20} />
  </button>

  {#if open}
    <div
      class="absolute left-0 top-full z-[var(--z-popover)] mt-2 w-[19rem] rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] p-3 shadow-xl"
    >
      <div class="flex flex-wrap gap-1.5">
        {#each AGENT_ICONS as opt (opt.name)}
          {@const Icon = opt.Icon}
          <button
            type="button"
            aria-pressed={value === opt.name}
            title={opt.name}
            class="flex h-8 w-8 items-center justify-center rounded-md border transition-colors {value ===
            opt.name
              ? 'border-primary/40 bg-primary/10 text-primary'
              : 'border-transparent text-base-content/55 hover:bg-base-200 hover:text-base-content'}"
            onclick={() => pick(opt.name)}
          >
            <Icon size={16} />
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<svelte:window onclick={handleOutside} />
