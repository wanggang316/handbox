<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import type { Model, ModelPricing } from "$lib/types/provider";
  import { Copy, Check, ExternalLink } from "lucide-svelte";
  import { openInBrowser } from "$lib/utils/browser";
  import { t } from "$lib/i18n";

  const props = $props<{
    open?: boolean;
    model?: Model | null;
    onClose?: () => void;
  }>();

  const open = $derived<boolean>(props.open ?? false);
  const model = $derived<Model | null>(props.model ?? null);
  const onClose = $derived<() => void>(props.onClose ?? (() => {}));

  let copied = $state(false);

  async function handleCopyModelId() {
    if (!model?.id) return;

    try {
      await navigator.clipboard.writeText(model.id);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (err) {
      console.error("Failed to copy model ID:", err);
    }
  }

  function formatLabel(key: string): string {
    return key
      .split("_")
      .map((segment) =>
        segment.length > 0
          ? segment.charAt(0).toUpperCase() + segment.slice(1).toLowerCase()
          : segment,
      )
      .join(" ");
  }

  function formatList(list?: string[] | null): string {
    if (!list || list.length === 0) return "";
    return list.map((item) => formatLabel(item)).join(", ");
  }

  function resolvePricingValue(
    pricing: ModelPricing | undefined,
    key: "input_text" | "output_text",
  ): string | null {
    if (!pricing) return null;
    const value = pricing[key];
    return value ?? null;
  }

  const promptPrice = $derived(resolvePricingValue(model?.pricing, "input_text"));
  const completionPrice = $derived(
    resolvePricingValue(model?.pricing, "output_text"),
  );
  const modelUrl = $derived(
    model?.url && model.url.trim().length > 0 ? model.url : null,
  );

  // 价格已由后端格式化，直接返回
  function formatPricePerMillion(value: string | null): string {
    return value || "";
  }

  type TableRow = {
    label: string;
    value: string;
    mono?: boolean;
    preserveWhitespace?: boolean;
  };

  const tableRows = $derived(
    (() => {
      const current = model;

      if (!current) {
        return [] as TableRow[];
      }

      const rows: TableRow[] = [];

      if (current.id) {
        rows.push({
          label: t("provider.modelId"),
          value: current.id,
          mono: true,
        });
      }

      // 使用后端格式化的字段
      if (current.display_context_length) {
        rows.push({
          label: t("provider.contextLength"),
          value: current.display_context_length,
        });
      }

      if (current.display_output_max_tokens) {
        rows.push({
          label: t("provider.maxOutputLength"),
          value: current.display_output_max_tokens,
        });
      }

      const inputPrice = formatPricePerMillion(promptPrice);
      if (inputPrice) {
        rows.push({
          label: t("provider.inputPrice"),
          value: inputPrice,
        });
      }

      const outputPrice = formatPricePerMillion(completionPrice);
      if (outputPrice) {
        rows.push({
          label: t("provider.outputPrice"),
          value: outputPrice,
        });
      }

      const supportedFeatures = formatList(current.supported_features);

      if (supportedFeatures) {
        rows.push({ label: t("provider.supportedFeatures"), value: supportedFeatures });
      }

      const inputModalities = formatList(current.input_modalities);

      if (inputModalities) {
        rows.push({ label: t("provider.inputModalities"), value: inputModalities });
      }

      const outputModalities = formatList(current.output_modalities);

      if (outputModalities) {
        rows.push({ label: t("provider.outputModalities"), value: outputModalities });
      }

      const supportedMethods = formatList(current.supported_chat_methods);
      if (supportedMethods) {
        rows.push({
          label: t("provider.supportedMethods"),
          value: supportedMethods,
          mono: true,
        });
      }

      const supportedParameters = current.supported_parameters
        ?.filter((p) => p && p.trim().length > 0)
        .join(", ");
      if (supportedParameters) {
        rows.push({
          label: t("provider.supportedParameters"),
          value: supportedParameters,
          mono: true,
        });
      }

      const description = current.description?.trim();
      if (description) {
        rows.push({
          label: t("common.description"),
          value: description,
          preserveWhitespace: true,
        });
      }

      return rows as TableRow[];
    })(),
  );

  async function handleOpenModelUrl() {
    if (!modelUrl) return;

    try {
      await openInBrowser(modelUrl);
    } catch (error) {
      console.error("Failed to open model url:", error);
    }
  }

</script>

<Modal {open} {onClose} title={model?.name ?? t("provider.modelInfo")}>
  <div
    class="mt-12 max-h-[70vh] max-w-xl w-full overflow-y-auto px-6 pb-6 space-y-6 text-sm text-base-content/90 scrollbar-padding relative"
  >
    {#if modelUrl}
      <button
        type="button"
        class="absolute top-0 right-6 inline-flex items-center justify-center h-8 w-8 rounded-full text-base-content/60 hover:text-primary transition-colors"
        title={t("provider.viewModelDetail")}
        onclick={handleOpenModelUrl}
      >
        <ExternalLink size={16} stroke-width={1.75} />
      </button>
    {/if}
    {#if model}
      <table
        class="w-full text-sm text-base-content/80 border-collapse table-fixed"
      >
        <tbody class="divide-y divide-[var(--hairline)]">
          {#each tableRows as row (row.label)}
            <tr>
              <td
                class="w-24 align-top py-2 pr-4 text-right text-xs uppercase tracking-wide text-base-content/60"
              >
                {row.label}
              </td>
              <td class="align-top py-[6px]">
                {#if row.preserveWhitespace}
                  <p
                    class="max-w-2xl text-xs leading-relaxed text-base-content/70 whitespace-pre-line"
                  >
                    {row.value}
                  </p>
                {:else if row.label === t("provider.modelId")}
                  <div class="flex items-center gap-2">
                    <span
                      class={`py-[0px] h-full text-xs text-base-content/80 ${row.mono ? "font-mono break-all" : ""}`}
                    >
                      {row.value}
                    </span>
                    <button
                      onclick={handleCopyModelId}
                      class="p-1 hover:bg-base-300 rounded transition-colors"
                      title={t("provider.copyModelId")}
                    >
                      {#if copied}
                        <Check size={12} class="text-success" />
                      {:else}
                        <Copy size={12} class="text-base-content/60" />
                      {/if}
                    </button>
                  </div>
                {:else}
                  <span
                    class={`py-[5px] h-full text-xs text-base-content/80 ${row.mono ? "font-mono break-all" : ""}`}
                  >
                    {row.value}
                  </span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="text-base-content/70">{t("provider.emptyModelInfo")}</div>
    {/if}
  </div>
</Modal>

<style>
  /* .scrollbar-padding {
    padding-bottom: 1.5rem;
    scrollbar-gutter: stable both-edges;
  } */

  :global(.scrollbar-padding::-webkit-scrollbar) {
    width: 6px;
  }

  :global(.scrollbar-padding::-webkit-scrollbar-track) {
    margin-bottom: 15px;
    background: transparent;
  }

  :global(.scrollbar-padding::-webkit-scrollbar-thumb) {
    background: color-mix(in oklch, var(--base-content) 15%, transparent);
    border-radius: 3px;
  }

  :global(.scrollbar-padding::-webkit-scrollbar-thumb:hover) {
    background: color-mix(in oklch, var(--base-content) 25%, transparent);
  }
</style>
