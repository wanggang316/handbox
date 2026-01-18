<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SelectRow, TableBaseRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { providerActions, providerState } from "$lib/states/provider.svelte";

  const targetLanguageOptions = [
    { value: "system", label: "跟随系统" },
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
    { value: "ja-JP", label: "日本語" },
    { value: "ko-KR", label: "한국어" },
    { value: "fr-FR", label: "Français" },
    { value: "de-DE", label: "Deutsch" },
    { value: "es-ES", label: "Español" },
    { value: "it-IT", label: "Italiano" },
    { value: "ru-RU", label: "Русский" },
    { value: "pt-BR", label: "Português (BR)" },
    { value: "ar-SA", label: "العربية" },
    { value: "custom", label: "自定义" },
  ];

  let providerOptions = $state<{ value: string; label: string }[]>([]);
  let modelOptions = $state<{ value: string; label: string }[]>([]);

  let providerId = $state("");
  let modelId = $state("");
  let targetLanguage = $state("system");
  let customTargetLanguage = $state("");

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);

  function canSaveTranslation(): boolean {
    return Boolean(providerId && modelId);
  }

  async function loadProviders() {
    try {
      await providerActions.loadProvidersWithModels(false);
      providerOptions = providerState.providersWithModels
        .filter((provider) => Boolean(provider.id))
        .map((provider) => ({
          value: provider.id ?? "",
          label: provider.name,
        }));
    } catch (error) {
      console.error("Failed to load providers:", error);
      errorMessage = "加载供应商失败";
    }
  }

  function refreshModelOptions() {
    const provider = providerState.providersWithModels.find(
      (item) => item.id === providerId
    );
    modelOptions =
      provider?.models.map((model) => ({
        value: model.id,
        label: model.name,
      })) ?? [];
  }

  async function updateTranslationSetting(data: Record<string, unknown>) {
    try {
      if (!canSaveTranslation()) {
        errorMessage = "请先选择翻译供应商与模型";
        return;
      }
      await settingsState.updateSettings({
        section: "translation",
        data,
      });
      errorMessage = null;
    } catch (error) {
      console.error("更新单词本设置失败:", error);
      errorMessage = "保存设置失败";
    }
  }

  async function handleProviderChange(value: string) {
    providerId = value;
    refreshModelOptions();
    const defaultModelId = modelOptions[0]?.value || "";
    if (!defaultModelId) {
      errorMessage = "该供应商暂无可用模型";
      modelId = "";
      return;
    }
    modelId = defaultModelId;
    await updateTranslationSetting({
      providerId: providerId || null,
      modelId: modelId || null,
    });
  }

  async function handleModelChange(value: string) {
    modelId = value;
    await updateTranslationSetting({
      providerId: providerId || null,
      modelId: modelId || null,
    });
  }

  async function handleTargetLanguageChange(value: string) {
    targetLanguage = value;
    if (value !== "custom") {
      await updateTranslationSetting({ targetLanguage: value });
    }
  }

  async function handleCustomTargetChange(value: string) {
    customTargetLanguage = value;
    if (targetLanguage === "custom") {
      await updateTranslationSetting({
        targetLanguage: customTargetLanguage || "system",
      });
    }
  }

  onMount(async () => {
    try {
      isLoading = true;
      await settingsState.loadSettings();
      await loadProviders();

      const translation = settingsState.settings?.translation;
      providerId = translation?.providerId || providerOptions[0]?.value || "";
      refreshModelOptions();
      modelId =
        translation?.modelId ||
        modelOptions[0]?.value ||
        "";

      const savedTarget = translation?.targetLanguage || "system";
      const hasPreset = targetLanguageOptions.some(
        (option) => option.value === savedTarget
      );
      if (hasPreset) {
        targetLanguage = savedTarget;
      } else {
        targetLanguage = "custom";
        customTargetLanguage = savedTarget;
      }
    } catch (error) {
      console.error("加载单词本设置失败:", error);
      errorMessage = "加载设置失败";
    } finally {
      isLoading = false;
    }
  });
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  {#if errorMessage}
    <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
      {errorMessage}
    </div>
  {/if}

  <TableGroup>
    <SelectRow
      label="翻译供应商"
      options={providerOptions}
      bind:selectedValue={providerId}
      onSelect={(value) => handleProviderChange(value)}
      disabled={providerOptions.length === 0}
    />
    <SelectRow
      label="翻译模型"
      options={modelOptions}
      bind:selectedValue={modelId}
      onSelect={(value) => handleModelChange(value)}
      disabled={modelOptions.length === 0}
    />
    <SelectRow
      label="翻译目标语言"
      options={targetLanguageOptions}
      bind:selectedValue={targetLanguage}
      onSelect={(value) => handleTargetLanguageChange(value)}
    />
    {#if targetLanguage === "custom"}
      <TableBaseRow label="自定义语言" py="2">
        <div class="flex flex-col items-end flex-1">
          <input
            class="w-full text-sm text-right text-base-content border-none outline-none p-1"
            placeholder="输入语言标签，如 en-US"
            bind:value={customTargetLanguage}
            oninput={(event) =>
              handleCustomTargetChange(
                (event.target as HTMLInputElement).value
              )}
          />
        </div>
      </TableBaseRow>
    {/if}
  </TableGroup>

  {#if !isLoading && providerOptions.length === 0}
    <div class="text-sm text-base-content/60">
      暂无可用供应商，请先在“模型”中添加并启用。
    </div>
  {/if}
</div>
