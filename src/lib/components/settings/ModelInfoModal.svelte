<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import type { Model, ModelPricing } from "$lib/types/provider";
  import { Copy, Check } from "lucide-svelte";

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

  function formatTokensAsK(value?: number | null): string {
    if (typeof value !== "number" || !Number.isFinite(value) || value === 0) {
      return "";
    }

    // 小于 1024 时显示实际数值，不加单位
    if (value < 1024) {
      return value.toString();
    }

    const thousands = value / 1024;
    const formatter = new Intl.NumberFormat(undefined, {
      maximumFractionDigits: thousands >= 10 ? 0 : 1,
    });

    return `${formatter.format(thousands)}K`;
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
    if (!list || list.length === 0) return "N/A";
    return list.map((item) => formatLabel(item)).join(", ");
  }

  function resolvePricingValue(
    pricing: ModelPricing | undefined,
    key: string,
  ): string | null {
    if (!pricing) return null;

    const entry = Object.entries(pricing).find(
      ([entryKey]) => entryKey.toLowerCase() === key.toLowerCase(),
    );

    if (!entry) return null;

    const [, value] = entry;
    if (value === undefined || value === null) {
      return null;
    }

    return typeof value === "string" ? value : value.toString();
  }

  const promptPrice = $derived(resolvePricingValue(model?.pricing, "prompt"));
  const completionPrice = $derived(
    resolvePricingValue(model?.pricing, "completion"),
  );

  function formatPricePerMillion(raw: string | null): string {
    if (!raw) {
      return "";
    }

    const numeric = Number(raw);
    if (!Number.isFinite(numeric) || numeric === 0) {
      return "";
    }

    const perMillion = numeric * 1_000_000;
    const formatter = new Intl.NumberFormat(undefined, {
      maximumFractionDigits: perMillion >= 100 ? 0 : perMillion >= 1 ? 2 : 4,
    });

    return `$${formatter.format(perMillion)}/M Token`;
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

      rows.push({
        label: "模型 ID",
        value: current.id || "N/A",
        mono: true,
      });

      const contextLength = formatTokensAsK(current.context_length);
      if (contextLength) {
        rows.push({
          label: "上下文长度",
          value: contextLength,
        });
      }

      const outputTokenLimit = formatTokensAsK(current.output_token_limit);
      if (outputTokenLimit) {
        rows.push({
          label: "最大输出长度",
          value: outputTokenLimit,
        });
      }

      const inputPrice = formatPricePerMillion(promptPrice);
      if (inputPrice) {
        rows.push({
          label: "输入价格",
          value: inputPrice,
        });
      }

      const outputPrice = formatPricePerMillion(completionPrice);
      if (outputPrice) {
        rows.push({
          label: "输出价格",
          value: outputPrice,
        });
      }

      const supportedFeatures =
        current.supported_features && current.supported_features.length > 0
          ? formatList(current.supported_features)
          : "N/A";

      if (supportedFeatures !== "N/A") {
        rows.push({ label: "支持特性", value: supportedFeatures });
      }

      const inputModalities =
        current.input_modalities && current.input_modalities.length > 0
          ? formatList(current.input_modalities)
          : "N/A";

      if (inputModalities !== "N/A") {
        rows.push({ label: "输入模态", value: inputModalities });
      }

      const outputModalities =
        current.output_modalities && current.output_modalities.length > 0
          ? formatList(current.output_modalities)
          : "N/A";

      if (outputModalities !== "N/A") {
        rows.push({ label: "输出模态", value: outputModalities });
      }

      const description = current.description?.trim();
      rows.push({
        label: "描述",
        value: description && description.length > 0 ? description : "N/A",
        preserveWhitespace: true,
      });

      return rows as TableRow[];
    })(),
  );
</script>

<Modal {open} {onClose} title={model?.name ?? "模型信息"}>
  <div
    class="mt-12 max-h-[70vh] max-w-xl w-full overflow-y-auto px-6 pb-6 space-y-6 text-sm text-base-content/90 scrollbar-padding"
  >
    {#if model}
      <table
        class="w-full text-sm text-base-content/80 border-collapse table-fixed"
      >
        <tbody class="divide-y divide-base-200">
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
                {:else if row.label === "模型 ID"}
                  <div class="flex items-center gap-2">
                    <span
                      class={`py-[0px] h-full text-xs text-base-content/80 ${row.mono ? "font-mono break-all" : ""}`}
                    >
                      {row.value}
                    </span>
                    <button
                      onclick={handleCopyModelId}
                      class="p-1 hover:bg-base-200 rounded transition-colors"
                      title="复制模型 ID"
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
      <div class="text-base-content/70">暂无模型信息</div>
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
