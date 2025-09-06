<script lang="ts">
  import { onMount } from 'svelte';
  import { TableGroup, SwitchRow, DropDownRow } from '$lib/components/ui/table';
  import { settingsState } from '$lib/stores';
  import type { Theme, ThemeColor, Language } from '$lib/types/settings';

  // 外观样式选项
  const themeOptions = [
    { value: 'system', label: '跟随系统' },
    { value: 'light', label: '浅色主题' },
    { value: 'dark', label: '深色主题' }
  ];

  // 语言选项
  const languageOptions = [
    { value: 'zh-CN', label: '简体中文' },
    { value: 'en-US', label: 'English' }
  ];

  // 主题色选项
  const themeColorOptions = [
    { value: 'system', label: '跟随系统' },
    { value: 'blue', label: '蓝色' },
    { value: 'green', label: '绿色' },
    { value: 'red', label: '红色' },
    { value: 'yellow', label: '黄色' },
    { value: 'purple', label: '紫色' },
    { value: 'orange', label: '橙色' },
    { value: 'pink', label: '粉色' },
    { value: 'brown', label: '棕色' }
  ];

  // 本地状态
  let theme: Theme = 'system';
  let language: Language = 'zh-CN';
  let themeColor: ThemeColor = 'system';
  let autoScroll: boolean = true;

  // 加载设置
  onMount(async () => {
    try {
      await settingsState.loadSettings();
      if (settingsState.settings?.general) {
        theme = settingsState.settings.general.theme;
        language = settingsState.settings.general.language;
        themeColor = settingsState.settings.general.themeColor;
        autoScroll = settingsState.settings.general.autoScroll;
      }
    } catch (error) {
      console.error('加载通用设置失败:', error);
    }
  });

  // 更新设置的通用函数
  async function updateGeneralSetting(key: string, value: any) {
    try {
      await settingsState.updateSettings({
        section: 'general',
        data: { [key]: value }
      });
    } catch (error) {
      console.error(`更新${key}设置失败:`, error);
    }
  }

  // 处理主题变更
  function handleThemeChange(value: string) {
    theme = value as Theme;
    updateGeneralSetting('theme', theme);
  }

  // 处理语言变更
  function handleLanguageChange(value: string) {
    language = value as Language;
    updateGeneralSetting('language', language);
  }

  // 处理主题色变更
  function handleThemeColorChange(value: string) {
    themeColor = value as ThemeColor;
    updateGeneralSetting('themeColor', themeColor);
  }

  // 处理自动下滑变更
  function handleAutoScrollChange(checked: boolean) {
    autoScroll = checked;
    updateGeneralSetting('autoScroll', autoScroll);
  }
</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup>
    <DropDownRow
      label="外观样式"
      options={themeOptions}
      bind:selectedValue={theme}
      onSelect={(value) => handleThemeChange(value)}
    />
    
    <DropDownRow
      label="语言"
      options={languageOptions}
      bind:selectedValue={language}
      onSelect={(value) => handleLanguageChange(value)}
    />
    
    <DropDownRow
      label="主题色"
      options={themeColorOptions}
      bind:selectedValue={themeColor}
      onSelect={(value) => handleThemeColorChange(value)}
    />
    
    <SwitchRow
      label="聊天界面自动下滑"
      bind:checked={autoScroll}
      description=""
    />
  </TableGroup>
</div>


