<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeft } from "@lucide/svelte";
  import GenUiEditor from "$lib/components/genui/GenUiEditor.svelte";
  import { genuiActions } from "$lib/states/genui.svelte";
  import type { GenUi } from "$lib/types";

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let genui = $state<GenUi | null>(null);

  const genuiId = $derived($page.params.id);

  async function loadDetail() {
    if (!genuiId) {
      errorMessage = "无效的 GenUI ID";
      return;
    }
    try {
      isLoading = true;
      errorMessage = null;
      genui = await genuiActions.getGenui(genuiId);
    } catch (error) {
      console.error("Failed to load GenUI detail:", error);
      errorMessage = "加载 GenUI 失败";
    } finally {
      isLoading = false;
    }
  }

  onMount(loadDetail);
</script>

{#if isLoading}
  <div class="h-full flex items-center justify-center">
    <div
      class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
    ></div>
  </div>
{:else if genui}
  {#key genui.id}
    <GenUiEditor {genui} />
  {/key}
{:else}
  <div class="h-full flex flex-col gap-4 p-6">
    <button
      class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mt-12"
      onclick={() => goto("/agents?tab=genui")}
    >
      <ArrowLeft size={14} />
      返回列表
    </button>
    <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
      {errorMessage ?? "未找到该 GenUI"}
    </div>
  </div>
{/if}
