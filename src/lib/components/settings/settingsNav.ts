/**
 * Single source of truth for settings navigation, shared by SettingsSidebar and
 * the settings layout header.
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
  Anchor,
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

// Reads t() on every call, so callers wrapping this in $derived recompute on language switch.
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
        { id: "hooks", title: t("settings.sidebar.hooks"), icon: Anchor, url: "/settings/hooks" },
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

/** Matches by route prefix, so sub-routes like /settings/models/provider/x resolve to models. */
export function findSettingsNavItem(
  pathname: string,
): SettingsNavItem | undefined {
  return getSettingsNavGroups()
    .flatMap((g) => g.items)
    .find((i) => pathname.startsWith(i.url));
}
