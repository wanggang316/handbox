<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow } from "$lib/components/ui/table";
  import { settingsState, uiState } from "$lib/states";
  import { listOpenInTargetsCached, type OpenInTarget } from "$lib/api/openIn";
  import { t } from "$lib/i18n";
  import { isMacOS } from "$lib/utils/tauri";
  import type { Theme, Language } from "$lib/types/settings";

  // Vibrancy is a macOS-only effect; the row is hidden elsewhere.
  const showSidebarVibrancy = isMacOS();

  // Derived so labels recompute on language change
  const themeOptions = $derived([
    { value: "system", label: t("settings.general.theme.system") },
    { value: "light", label: t("settings.general.theme.light") },
    { value: "dark", label: t("settings.general.theme.dark") },
  ]);

  // Each language shows as its own endonym; never translated
  const languageOptions = [
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
  ];

  let theme = $state<Theme>("system");
  let language = $state<Language>("zh-CN");
  let autoScroll = $state<boolean>(true);
  let sidebarVibrancy = $state<boolean>(true);
  let messageNav = $state<boolean>(true);

  // Default "Open in ..." app. Lives in the agent section (the session header
  // and the per-project panel both read it), but it is an app-wide preference,
  // so this is where it is set. "" = no explicit choice.
  const AUTO_EDITOR_VALUE = "";
  let defaultEditorId = $state(AUTO_EDITOR_VALUE);
  let openInTargets = $state<OpenInTarget[]>([]);

  // Finder is always installed and is the fallback, not a "default editor".
  const editorOptions = $derived([
    { value: AUTO_EDITOR_VALUE, label: t("settings.general.defaultEditorAuto") },
    ...openInTargets
      .filter((target) => target.kind !== "system")
      .map((target) => ({ value: target.id, label: target.name })),
  ]);

  function syncFromSettings(): void {
    defaultEditorId =
      settingsState.settings?.agent?.defaultEditorId ?? AUTO_EDITOR_VALUE;
    if (!settingsState.settings?.general) return;
    theme = settingsState.settings.general.theme;
    language = settingsState.settings.general.language;
    autoScroll = settingsState.settings.general.autoScroll;
    sidebarVibrancy = settingsState.settings.general.sidebarVibrancy ?? true;
    messageNav = settingsState.settings.general.messageNav ?? true;

    uiState.setTheme(theme);
    uiState.setLanguage(language);
    uiState.setSidebarVibrancy(sidebarVibrancy);
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
        console.error("加载通用设置失败:", error);
      });

    // Cached app probe: the row shows only the "auto" option until it lands.
    listOpenInTargetsCached()
      .then((targets) => (openInTargets = targets))
      .catch((error) => {
        console.error("检测可用编辑器失败:", error);
      });
  });

  async function updateGeneralSetting(key: string, value: any) {
    try {
      await settingsState.updateSettings({
        section: "general",
        data: { [key]: value },
      });
    } catch (error) {
      console.error(`更新${key}设置失败:`, error);
    }
  }

  function handleThemeChange(value: string) {
    theme = value as Theme;
    uiState.setTheme(theme);
    updateGeneralSetting("theme", theme);
  }

  function handleLanguageChange(value: string) {
    language = value as Language;
    uiState.setLanguage(language);
    updateGeneralSetting("language", language);
  }

  function handleAutoScrollChange(checked: boolean) {
    autoScroll = checked;
    updateGeneralSetting("autoScroll", autoScroll);
  }

  function handleMessageNavChange(checked: boolean) {
    messageNav = checked;
    updateGeneralSetting("messageNav", messageNav);
  }

  function handleSidebarVibrancyChange(checked: boolean) {
    sidebarVibrancy = checked;
    uiState.setSidebarVibrancy(sidebarVibrancy);
    updateGeneralSetting("sidebarVibrancy", sidebarVibrancy);
  }

  async function handleDefaultEditorChange(value: string) {
    defaultEditorId = value;
    try {
      await settingsState.updateSettings({
        section: "agent",
        data: { defaultEditorId: value || null },
      });
    } catch (error) {
      console.error("更新默认编辑器设置失败:", error);
    }
  }

</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <TableGroup title={t("settings.general.section")}>
    <SelectRow
      label={t("settings.general.appearance")}
      description={t("settings.general.appearanceDesc")}
      options={themeOptions}
      bind:selectedValue={theme}
      onSelect={(value) => handleThemeChange(value)}
    />

    <SelectRow
      label={t("settings.general.language")}
      description={t("settings.general.languageDesc")}
      options={languageOptions}
      bind:selectedValue={language}
      onSelect={(value) => handleLanguageChange(value)}
    />

    {#if showSidebarVibrancy}
      <SwitchRow
        label={t("settings.general.sidebarVibrancy")}
        description={t("settings.general.sidebarVibrancyDesc")}
        bind:checked={sidebarVibrancy}
        onChange={handleSidebarVibrancyChange}
      />
    {/if}

    <SwitchRow
      label={t("settings.general.autoScroll")}
      description={t("settings.general.autoScrollDesc")}
      bind:checked={autoScroll}
      onChange={handleAutoScrollChange}
    />

    <SwitchRow
      label={t("settings.general.messageNav")}
      description={t("settings.general.messageNavDesc")}
      bind:checked={messageNav}
      onChange={handleMessageNavChange}
    />
  </TableGroup>

  <TableGroup title={t("settings.general.editorSection")}>
    <SelectRow
      label={t("settings.general.defaultEditor")}
      description={t("settings.general.defaultEditorDesc")}
      options={editorOptions}
      bind:selectedValue={defaultEditorId}
      onSelect={(value) => handleDefaultEditorChange(value)}
    />
  </TableGroup>
</div>
