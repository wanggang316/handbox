/**
 * 设置导航的共享数据源：SettingsSidebar（左栏）与 settings/+layout（页头标题）
 * 共用，保证条目/分组/文案一处维护。
 */
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
} from "@lucide/svelte";
import type { Icon as IconType } from "@lucide/svelte";
import { t } from "$lib/i18n";

export interface SettingsNavItem {
  id: string;
  title: string;
  icon: typeof IconType;
  url: string;
}

export interface SettingsNavGroup {
  id: string;
  title: string;
  items: SettingsNavItem[];
}

// 每次调用即时取 t()：调用方包在 $derived 里即可随语言切换重算。
export function getSettingsNavGroups(): SettingsNavGroup[] {
  return [
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
  ];
}

/** 按路由前缀匹配当前导航项（子路由如 /settings/models/provider/x 归属 models）。 */
export function findSettingsNavItem(
  pathname: string,
): SettingsNavItem | undefined {
  return getSettingsNavGroups()
    .flatMap((g) => g.items)
    .find((i) => pathname.startsWith(i.url));
}
