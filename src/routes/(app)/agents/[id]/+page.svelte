<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeft } from "@lucide/svelte";
  import AgentEditor from "$lib/components/agent/AgentEditor.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { agentActions } from "$lib/states/agent.svelte";
  import { t } from "$lib/i18n";
  import type { Agent } from "$lib/types";

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let agent = $state<Agent | null>(null);

  const agentId = $derived($page.params.id);

  async function loadDetail() {
    if (!agentId) {
      errorMessage = t("agent.manage.loadFailed");
      return;
    }
    try {
      isLoading = true;
      errorMessage = null;
      agent = await agentActions.getAgent(agentId);
    } catch (error) {
      console.error("Failed to load agent detail:", error);
      errorMessage = t("agent.manage.loadFailed");
    } finally {
      isLoading = false;
    }
  }

  onMount(loadDetail);
</script>

{#if isLoading}
  <div class="h-full flex items-center justify-center">
    <Spinner size={28} />
  </div>
{:else if agent}
  {#key agent.id}
    <AgentEditor {agent} />
  {/key}
{:else}
  <div class="h-full flex flex-col gap-4 px-6 pt-12">
    <div class="mx-auto w-full max-w-5xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit"
        onclick={() => goto("/agents")}
      >
        <ArrowLeft size={14} />
        {t("agent.form.backToList")}
      </button>
      <div class="mt-4 p-3 rounded-lg bg-error/10 text-error text-sm">
        {errorMessage ?? t("agent.manage.loadFailed")}
      </div>
    </div>
  </div>
{/if}
