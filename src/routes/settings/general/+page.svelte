<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow } from "$lib/components/ui/table";
  import { settingsState, uiState } from "$lib/states";
  import { t } from "$lib/i18n";
  import type { Theme, Language } from "$lib/types/settings";

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

  function syncFromSettings(): void {
    if (!settingsState.settings?.general) return;
    theme = settingsState.settings.general.theme;
    language = settingsState.settings.general.language;
    autoScroll = settingsState.settings.general.autoScroll;

    uiState.setTheme(theme);
    uiState.setLanguage(language);
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

    <SwitchRow
      label={t("settings.general.autoScroll")}
      description={t("settings.general.autoScrollDesc")}
      bind:checked={autoScroll}
      onChange={handleAutoScrollChange}
    />
  </TableGroup>
</div>
