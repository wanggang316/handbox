<script lang="ts">
  /**
   * Settings-row trigger for the shared model picker.
   *
   * Shows the selected model (provider icon + name) or a placeholder, and opens
   * `ModelSelectModal` on click. Selection is reported upward; the component
   * holds no persisted state of its own.
   */
  import { ChevronsUpDown } from "@lucide/svelte";
  import ModelSelectModal from "$lib/components/agentsession/ModelSelectModal.svelte";
  import { getProviderIconById } from "$lib/states/provider.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    /** Only a resolved (runnable) model; a dangling default shows the placeholder. */
    selectedModel?: ModelWithProvider | null;
    /** Shown when nothing is selected (e.g. "not selected"). */
    placeholder: string;
    onModelSelect: (model: ModelWithProvider) => void;
  }

  let { selectedModel = null, placeholder, onModelSelect }: Props = $props();

  let modalOpen = $state(false);

  const providerIcon = $derived(
    selectedModel ? getProviderIconById(selectedModel.provider_id) : undefined,
  );
</script>

<button
  type="button"
  class="flex h-8 items-center gap-1.5 rounded-md border border-base-300 px-2.5 text-sm text-base-content/80 hover:bg-base-200/60 transition-colors"
  title={selectedModel?.name ?? placeholder}
  onclick={() => (modalOpen = true)}
>
  {#if selectedModel}
    {#if providerIcon}
      <img
        src={providerIcon}
        alt={selectedModel.providerName}
        class="h-4 w-4 shrink-0 rounded object-contain"
      />
    {/if}
    <span class="max-w-[240px] truncate">{selectedModel.name}</span>
  {:else}
    <span class="max-w-[240px] truncate text-base-content/60">{placeholder}</span>
  {/if}
  <ChevronsUpDown size={13} class="shrink-0 opacity-60" />
</button>

<ModelSelectModal bind:open={modalOpen} {selectedModel} {onModelSelect} />
