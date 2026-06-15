<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow } from "$lib/components/ui/table";
  import { settingsState, uiState } from "$lib/states";
  import type { Theme, Language } from "$lib/types/settings";

  // 外观样式选项
  const themeOptions = [
    { value: "system", label: "跟随系统" },
    { value: "light", label: "浅色主题" },
    { value: "dark", label: "深色主题" },
  ];

  // 语言选项
  const languageOptions = [
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
  ];

  // 本地状态
  let theme: Theme = "system";
  let language: Language = "zh-CN";
  let autoScroll: boolean = true;

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
      label="外观样式"
      options={themeOptions}
      bind:selectedValue={theme}
      onSelect={(value) => handleThemeChange(value)}
    />

    <SelectRow
      label="语言"
      options={languageOptions}
      bind:selectedValue={language}
      onSelect={(value) => handleLanguageChange(value)}
    />

    <SwitchRow
      label="聊天界面自动下滑"
      bind:checked={autoScroll}
      description=""
      onChange={handleAutoScrollChange}
    />
  </TableGroup>
</div>
