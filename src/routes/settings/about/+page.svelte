<script lang="ts">
  import { onMount } from "svelte";
  import DefaultRow from "$lib/components/ui/table/DefaultRow.svelte";
  import SelectRow from "$lib/components/ui/table/SelectRow.svelte";
  import SwitchRow from "$lib/components/ui/table/SwitchRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
  import {
    updateState,
    PROMPT_INTERVALS,
    type PromptInterval,
  } from "$lib/states/update.svelte";
  import { openInBrowser } from "$lib/utils";
  import { t } from "$lib/i18n";

  onMount(() => {
    updateState.load().catch((error) => {
      console.error("Failed to load update state:", error);
    });
  });

  const checkValue = $derived(
    updateState.status === "checking"
      ? t("settings.about.checking")
      : updateState.status === "available"
        ? t("settings.about.updateAvailable", {
            version: updateState.info?.version ?? "",
          })
        : t("settings.about.currentVersion", {
            version: updateState.currentVersion,
          })
  );

  function handleCheckVersion(): void {
    if (updateState.hasUpdate) {
      updateState.openDialog();
      return;
    }
    updateState
      .checkForUpdate({ notifyNoUpdate: true, openOnFound: true })
      .catch((error) => {
        console.error("Failed to check for update:", error);
      });
  }

  function handleAutoCheckChange(value: boolean): void {
    updateState.setAutoCheck(value);
  }

  const promptIntervalOptions = $derived(
    PROMPT_INTERVALS.map((value) => ({
      value,
      label: t(`settings.about.promptInterval.${value}`),
    }))
  );

  function handlePromptIntervalChange(value: string): void {
    updateState.setPromptInterval(value as PromptInterval);
  }

  async function handleOpenChangelog(): Promise<void> {
    try {
      await openInBrowser(
        "https://github.com/wanggang316/handbox/blob/main/CHANGELOG.md"
      );
    } catch (error) {
      console.error("Failed to open changelog:", error);
    }
  }

  async function handleOpenOfficalSite(): Promise<void> {
    try {
      await openInBrowser("https://handbox.ai");
    } catch (error) {
      console.error("Failed to open official site:", error);
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <div class="rounded-xl overflow-hidden">
    <TableGroup title={t("settings.about.softwareUpdate")}>
      <SwitchRow
        label={t("settings.about.autoCheck")}
        description={t("settings.about.autoCheckHint")}
        checked={updateState.autoCheck}
        onChange={handleAutoCheckChange}
      />
      <SelectRow
        label={t("settings.about.promptInterval")}
        description={t("settings.about.promptIntervalDesc")}
        options={promptIntervalOptions}
        selectedValue={updateState.promptInterval}
        disabled={!updateState.autoCheck}
        onSelect={handlePromptIntervalChange}
      />
      <DefaultRow
        label={t("settings.about.checkUpdate")}
        value={checkValue}
        onclick={handleCheckVersion}
      />
    </TableGroup>
  </div>

  <div class="rounded-xl overflow-hidden">
    <TableGroup title={t("settings.about.title")}>
      <DefaultRow label={t("settings.about.changelog")} onclick={handleOpenChangelog} />
      <DefaultRow label={t("settings.about.officialSite")} onclick={handleOpenOfficalSite} />
    </TableGroup>
  </div>
</div>

<UpdateDialog />
