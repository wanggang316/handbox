<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import Menu from "$lib/components/ui/Menu.svelte";
  import { t } from "$lib/i18n";
  import { navigationState } from "$lib/states/navigation.svelte";
  import { getSettingsNavGroups } from "./settingsNav";
  import type { Snippet } from "svelte";

  let { footer }: { footer?: Snippet } = $props();

  const groups = $derived(getSettingsNavGroups());

  const allItems = $derived(groups.flatMap((g) => g.items));

  const currentItemId = $derived(
    allItems.find((i) => $page.url.pathname.startsWith(i.url))?.id ?? "account"
  );

  function navTo(id: string) {
    const target = allItems.find((i) => i.id === id);
    if (target) goto(target.url);
  }
</script>

<div class="h-full flex flex-col p-0 pt-10 overflow-hidden">
  <!-- 返回应用：设置在主窗口内渲染，回到进入设置前的主界面路由 -->
  <div class="px-2 pb-1">
    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-lg p-2 text-[13px] text-base-content/70 hover:text-base-content hover:bg-base-300 transition-colors"
      onclick={() => goto(navigationState.backTarget)}
    >
      <ArrowLeft size={15} />
      {t("settings.sidebar.backToApp")}
    </button>
  </div>

  <!-- 分组导航 -->
  <div class="flex-1 overflow-y-auto pb-2">
    {#each groups as group (group.id)}
      <div class="pt-3 first:pt-1">
        <div class="px-4 pb-1 text-xs text-base-content/45">{group.title}</div>
        <Menu
          title=""
          items={group.items}
          onItemClick={(item) => navTo(item.id)}
          activeId={currentItemId}
        />
      </div>
    {/each}
  </div>
  {@render footer?.()}
</div>
