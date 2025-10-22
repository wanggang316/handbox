<script lang="ts">
  import { chatActions, currentChatModel } from "$lib/states/chat.svelte";
  import { getProviderIconById } from "$lib/states/provider.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import { ChevronsUpDown } from "lucide-svelte";
  import ChatModelSelectModal from "../ChatModelSelectModal.svelte";

  const currentModel = $derived<ModelWithProvider | undefined>(
    currentChatModel().model
  );

  const providerIcon = $derived(
    currentModel?.provider_id
      ? getProviderIconById(currentModel.provider_id)
      : undefined
  );

  let showModelModal = $state(false);

  function handleModelSelect(model: ModelWithProvider) {
    chatActions.updateChatModel(model.id, model.provider_id);
    showModelModal = false;
  }
</script>

<button
  class="w-full rounded-2xl bg-base-200 px-3 py-4 border border-base-200 hover:bg-base-300"
  type="button"
  onclick={() => (showModelModal = true)}
>
  {#if currentModel}
    <div class="flex items-start justify-between gap-2">
      <div class="space-y-1 pb-1 flex-1 flex flex-col text-left">
        <div class="flex flex-row justify-start items-center gap-2">
          {#if providerIcon}
            <img
              src={providerIcon}
              alt={currentModel?.providerName ?? "模型供应商"}
              class="h-4 w-4 rounded-md object-contain"
            />
          {/if}
          <p class="text-xs text-base-content/50">
            {currentModel?.providerName ?? "模型供应商"}
          </p>
        </div>

        <div class="text-md text-base-content">
          {currentModel ? currentModel.name : "未选择模型"}
        </div>
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-base-content/60"
        >
          {#if currentModel?.id}
            <span class="font-mono text-[11px] text-base-content/50">
              {currentModel.id}
            </span>
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="flex flex-row justify-between items-center px-2">
      <p class="text-left text-sm leading-relaxed text-base-content">
        选择一个模型以开始对话
      </p>
      <ChevronsUpDown size={14} />
    </div>
  {/if}
</button>

<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={currentModel ?? null}
  onModelSelect={handleModelSelect}
/>
