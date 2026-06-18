<script lang="ts">
  import { onMount } from "svelte";
  import DefaultRow from "$lib/components/ui/table/DefaultRow.svelte";
  import SwitchRow from "$lib/components/ui/table/SwitchRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import { openInBrowser } from "$lib/utils";
  import { t } from "$lib/i18n";

  onMount(() => {
    updateState.load().catch((error) => {
      console.error("Failed to load update state:", error);
    });
  });

  // 检查更新行的状态文案
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

  async function handleOpenChangelog(): Promise<void> {
    try {
      await openInBrowser(
        "https://github.com/wanggang/handbox/blob/main/CHANGELOG.md"
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

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <!-- 软件更新 -->
  <div class="rounded-xl overflow-hidden">
    <TableGroup title={t("settings.about.softwareUpdate")}>
      <SwitchRow
        label={t("settings.about.autoCheck")}
        description={t("settings.about.autoCheckHint")}
        checked={updateState.autoCheck}
        onChange={handleAutoCheckChange}
      />
      <DefaultRow
        label={t("settings.about.checkUpdate")}
        value={checkValue}
        onclick={handleCheckVersion}
      />
    </TableGroup>
  </div>

  <!-- 关于 -->
  <div class="rounded-xl overflow-hidden">
    <TableGroup title={t("settings.about.title")}>
      <DefaultRow label={t("settings.about.changelog")} onclick={handleOpenChangelog} />
      <DefaultRow label={t("settings.about.officialSite")} onclick={handleOpenOfficalSite} />
    </TableGroup>
  </div>
</div>

<UpdateDialog />
