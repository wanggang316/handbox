<script lang="ts">
  import {
    User,
    Palette,
    Brain,
    Zap,
    Sparkles,
    Keyboard,
    Info,
    MousePointerClick,
    LayoutGrid,
    Wrench,
    ArrowLeft,
    Search,
  } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import Menu from "$lib/components/ui/Menu.svelte";
  import { t } from "$lib/i18n";
  import { navigationState } from "$lib/states/navigation.svelte";
  import type { Snippet } from "svelte";

  let { footer }: { footer?: Snippet } = $props();

  type Item = { id: string; title: string; icon: any; url: string };
  type Group = { id: string; title: string; items: Item[] };

  let searchQuery = $state("");

  const groups: Group[] = $derived([
    {
      id: "personal",
      title: t("settings.sidebar.group.personal"),
      items: [
        { id: "account", title: t("settings.sidebar.account"), icon: User, url: "/settings/account" },
        { id: "general", title: t("settings.sidebar.general"), icon: Palette, url: "/settings/general" },
        { id: "shortcuts", title: t("settings.sidebar.shortcuts"), icon: Keyboard, url: "/settings/shortcuts" },
      ],
    },
    {
      id: "features",
      title: t("settings.sidebar.group.features"),
      items: [
        { id: "quicktools", title: t("settings.sidebar.quicktools"), icon: MousePointerClick, url: "/settings/quicktools" },
        { id: "models", title: t("settings.sidebar.models"), icon: Brain, url: "/settings/models" },
        { id: "agent-tools", title: t("settings.sidebar.agentTools"), icon: Wrench, url: "/settings/agent-tools" },
        { id: "mcp", title: "MCP", icon: Zap, url: "/settings/mcp" },
        { id: "skills", title: t("settings.sidebar.skills"), icon: Sparkles, url: "/settings/skills" },
      ],
    },
    {
      id: "other",
      title: t("settings.sidebar.group.other"),
      items: [
        { id: "components", title: t("settings.sidebar.components"), icon: LayoutGrid, url: "/settings/components" },
        { id: "about", title: t("settings.sidebar.about"), icon: Info, url: "/settings/about" },
      ],
    },
  ]);

  const allItems = $derived(groups.flatMap((g) => g.items));

  // 搜索过滤：按标题（不区分大小写），空组隐藏。
  const filteredGroups = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return groups;
    return groups
      .map((g) => ({
        ...g,
        items: g.items.filter((i) => i.title.toLowerCase().includes(q)),
      }))
      .filter((g) => g.items.length > 0);
  });

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

  <!-- 搜索设置 -->
  <div class="px-2 pb-2">
    <div class="relative">
      <Search
        size={14}
        class="absolute left-2.5 top-1/2 -translate-y-1/2 text-base-content/40 pointer-events-none"
      />
      <input
        bind:value={searchQuery}
        placeholder={t("settings.sidebar.search")}
        class="field w-full py-1.5 pl-8 pr-2 text-[13px]"
      />
    </div>
  </div>

  <!-- 分组导航 -->
  <div class="flex-1 overflow-y-auto pb-2">
    {#each filteredGroups as group (group.id)}
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
