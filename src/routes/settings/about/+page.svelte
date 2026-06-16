<script lang="ts">
  import { onMount } from "svelte";
  import DefaultRow from "$lib/components/ui/table/DefaultRow.svelte";
  import SwitchRow from "$lib/components/ui/table/SwitchRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import { openInBrowser } from "$lib/utils";

  onMount(() => {
    updateState.load().catch((error) => {
      console.error("Failed to load update state:", error);
    });
  });

  // 检查更新行的状态文案
  const checkValue = $derived(
    updateState.status === "checking"
      ? "检查中…"
      : updateState.status === "available"
        ? `发现新版本 v${updateState.info?.version ?? ""}`
        : `当前版本 v${updateState.currentVersion}`
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
    <TableGroup title="软件更新">
      <SwitchRow
        label="自动检查更新"
        description="启动时自动检查"
        checked={updateState.autoCheck}
        onChange={handleAutoCheckChange}
      />
      <DefaultRow
        label="检查更新"
        value={checkValue}
        onclick={handleCheckVersion}
      />
    </TableGroup>
  </div>

  <!-- 关于 -->
  <div class="rounded-xl overflow-hidden">
    <TableGroup title="关于">
      <DefaultRow label="更新日志" onclick={handleOpenChangelog} />
      <DefaultRow label="官方网站" onclick={handleOpenOfficalSite} />
    </TableGroup>
  </div>
</div>

<UpdateDialog />
