<script lang="ts">
  import InfoTooltip from "../InfoTooltip.svelte";

  interface Props {
    label?: string;
    description?: string; // subtitle under the left label (Linear-style settings row)
    icon?: any; // Lucide icon component
    layout?: "horizontal" | "vertical";
    py?: string;
    rightContent?: any; // content at the right of the header row
    helpText?: string; // shown as a question-mark tooltip icon
    error?: string; // field-level error rendered inline below the control
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
