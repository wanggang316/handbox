<script lang="ts">
  import DefaultRow from "$lib/components/ui/table/DefaultRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import { openInBrowser } from "$lib/utils";

    function handleCheckVersion(): void {
        throw new Error("Function not implemented.");
    }

    async function handleOpenChangelog(): Promise<void> {
        try {
            await openInBrowser("https://github.com/wanggang/handbox/blob/main/CHANGELOG.md");
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

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <div class="rounded-[20px] overflow-hidden">
    <TableGroup>
      <DefaultRow label="检查更新" value="当前版本 V1.0.0" onclick={handleCheckVersion} />
      <DefaultRow label="更新日志" onclick={handleOpenChangelog} />
      <DefaultRow label="官方网站" onclick={handleOpenOfficalSite} />
    </TableGroup>
  </div>
</div>
