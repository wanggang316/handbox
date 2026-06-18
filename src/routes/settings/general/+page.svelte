<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow } from "$lib/components/ui/table";
  import { settingsState, uiState } from "$lib/states";
  import { t } from "$lib/i18n";
  import type { Theme, Language } from "$lib/types/settings";

  // 外观样式选项（随语言切换重算）
  const themeOptions = $derived([
    { value: "system", label: t("settings.general.theme.system") },
    { value: "light", label: t("settings.general.theme.light") },
    { value: "dark", label: t("settings.general.theme.dark") },
  ]);

  // 语言选项：各语言以其自身名称（endonym）展示，不翻译
  const languageOptions = [
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
  ];

  // 本地状态（本文件已进入 runes 模式，需用 $state 才能保持双向绑定响应式）
  let theme = $state<Theme>("system");
  let language = $state<Language>("zh-CN");
  let autoScroll = $state<boolean>(true);

  // 加载设置
  onMount(async () => {
    try {
      await settingsState.loadSettings();
      if (settingsState.settings?.general) {
        theme = settingsState.settings.general.theme;
        language = settingsState.settings.general.language;
        autoScroll = settingsState.settings.general.autoScroll;

        uiState.setTheme(theme);
        uiState.setLanguage(language);
      }

    } catch (error) {
      console.error("加载通用设置失败:", error);
    }
  });

  // 更新设置的通用函数
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

  // 处理主题变更
  function handleThemeChange(value: string) {
    theme = value as Theme;
    uiState.setTheme(theme);
    updateGeneralSetting("theme", theme);
  }

  // 处理语言变更
  function handleLanguageChange(value: string) {
    language = value as Language;
    uiState.setLanguage(language);
    updateGeneralSetting("language", language);
  }

  // 处理自动下滑变更
  function handleAutoScrollChange(checked: boolean) {
    autoScroll = checked;
    updateGeneralSetting("autoScroll", autoScroll);
  }

</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup>
    <SelectRow
      label={t("settings.general.appearance")}
      options={themeOptions}
      bind:selectedValue={theme}
      onSelect={(value) => handleThemeChange(value)}
    />

    <SelectRow
      label={t("settings.general.language")}
      options={languageOptions}
      bind:selectedValue={language}
      onSelect={(value) => handleLanguageChange(value)}
    />

    <SwitchRow
      label={t("settings.general.autoScroll")}
      bind:checked={autoScroll}
      description=""
      onChange={handleAutoScrollChange}
    />
  </TableGroup>
</div>
