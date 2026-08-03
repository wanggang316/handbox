<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SelectRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { t } from "$lib/i18n";
  import type { TitleGenerationRule } from "$lib/types/settings";

  // Derived so labels recompute on language change
  const titleGenerationOptions = $derived([
    {
      value: "firstMessage",
      label: t("settings.session.titleGeneration.firstMessage"),
    },
    {
      value: "everyMessage",
      label: t("settings.session.titleGeneration.everyMessage"),
    },
    { value: "off", label: t("settings.session.titleGeneration.off") },
  ]);

  let titleGeneration = $state<TitleGenerationRule>("firstMessage");

  function syncFromSettings(): void {
    if (!settingsState.settings) return;
    titleGeneration =
      settingsState.settings.session?.titleGeneration ?? "firstMessage";
  }

  // Root layout preloaded settings: sync backfill so the first frame shows real
  // values (no default-value flicker).
  syncFromSettings();

  // Cold-start/deep-link fallback: resync once settings finish loading
  onMount(() => {
    settingsState
      .loadSettings()
      .then(syncFromSettings)
      .catch((error) => {
        console.error("加载会话设置失败:", error);
      });
  });

  async function handleTitleGenerationChange(value: string) {
    titleGeneration = value as TitleGenerationRule;
    try {
      await settingsState.updateSettings({
        section: "session",
        data: { titleGeneration },
      });
    } catch (error) {
      console.error("更新标题生成设置失败:", error);
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <TableGroup title={t("settings.session.section")}>
    <SelectRow
      label={t("settings.session.titleGeneration")}
      options={titleGenerationOptions}
      bind:selectedValue={titleGeneration}
      onSelect={(value) => handleTitleGenerationChange(value)}
    />
  </TableGroup>
</div>
