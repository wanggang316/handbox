<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    providerState,
    providerActions,
    providerStateActions,
    providerConfigs,
    getProviderIcon,
  } from "$lib/states/provider.svelte";
  import { Cpu } from "@lucide/svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { TableGroup } from "$lib/components/ui/table";
  import StatusLabelRow from "$lib/components/ui/table/StatusLabelRow.svelte";
  import AddProviderModal from "$lib/components/settings/AddProviderModal.svelte";
  import type { Provider } from "$lib/types/provider";
  import Button from "$lib/components/ui/Button.svelte";
  import { t } from "$lib/i18n";

  let showAddProviderModal = $state(false);

  onMount(async () => {
    try {
      await Promise.all([
        providerActions.loadProviderConfigs(),
        providerActions.loadProviders()
      ]);
    } catch (error) {
      console.error("Failed to load providers:", error);
    }
  });

  function handleProviderClick(provider: Provider) {
    goto(`/settings/models/provider/${provider.id}`);
  }

  function handleAddProvider() {
    showAddProviderModal = true;
  }

  $effect(() => {
    if (!showAddProviderModal) {
      // Closing the modal must clear any in-progress edit state
      providerStateActions.endEditProvider();
    }
  });

  function getProviderStatus(
    provider: Provider,
  ): "enabled" | "disabled" {
    return provider.enabled ? "enabled" : "disabled";
  }

  function getProviderStatusText(provider: Provider): string {
    return provider.enabled ? t("common.enabled") : t("common.disabled");
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">

  <!-- Spinner only on a true first load; with store data, render the list and
       refresh silently to avoid a list-spinner-list flash. -->
  {#if providerState.isLoading && providerState.providers.length === 0}
    <div class="flex items-center justify-center py-10">
      <Spinner size={28} />
    </div>
  {/if}

  <div class="rounded-xl overflow-hidden">
    <TableGroup>
      {#each providerState.providers as provider (provider.id)}
        <StatusLabelRow
          label={provider.name}
          iconSrc={getProviderIcon(provider)}
          icon={!getProviderIcon(provider)
            ? provider.name.charAt(0).toUpperCase()
            : undefined}
          isCustomProvider={![...providerConfigs.providers, ...providerConfigs.custom_providers].some(
            (t) => t.provider_type === provider.provider_type,
          )}
          status={getProviderStatus(provider)}
          statusText={getProviderStatusText(provider)}
          onclick={() => handleProviderClick(provider)}
        />
      {/each}

      {#if !providerState.isLoading && providerState.providers.length === 0}
        <div class="p-8 text-center">
          <Cpu class="h-12 w-12 text-base-content/50 mx-auto mb-4" />
          <p class="text-base text-base-content/70 mb-4">
            {t("provider.emptyHint")}
          </p>
          <Button variant="primary" size="sm" onclick={handleAddProvider}>
            {t("provider.addProvider")}
          </Button>
        </div>
      {/if}
    </TableGroup>
  </div>

  {#if providerState.providers.length > 0}
    <div>
      <Button variant="gray" size="sm" onclick={handleAddProvider}>
        {t("provider.addOtherProvider")}
      </Button>
    </div>
  {/if}
</div>

<AddProviderModal
  open={showAddProviderModal}
  onClose={() => showAddProviderModal = false}
/>
