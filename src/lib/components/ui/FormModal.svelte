<script lang="ts">
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { t } from "$lib/i18n";
  import type { Snippet } from "svelte";

  // Shared shell for create/edit dialogs: header (Modal traffic light + title) and a
  // footer action bar (hint/error left, cancel + primary right), so form modals stay
  // visually consistent.
  interface Props {
    open?: boolean;
    title?: string;
    onClose?: () => void;
    /** md = 560px single column; lg = 920px main area + right config pane (aside snippet). */
    size?: "md" | "lg";
    saving?: boolean;
    submitLabel?: string;
    cancelLabel?: string;
    submitDisabled?: boolean;
    onSubmit?: () => void;
    /** Persistent hint at the footer's left side. */
    hint?: string;
    /** Save error; shown instead of hint. */
    error?: string | null;
    children?: Snippet;
    /** Right config pane; rendered only when size="lg". */
    aside?: Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    onClose = () => {},
    size = "md",
    saving = false,
    submitLabel = "",
    cancelLabel = "",
    submitDisabled = false,
    onSubmit = () => {},
    hint = "",
    error = null,
    children,
    aside,
  }: Props = $props();

  const width = $derived(size === "lg" ? "w-[920px]" : "w-[560px]");
</script>

<Modal bind:open {title} {onClose}>
  <!-- lg uses a fixed height so the main area can flex-fill (e.g. a large textarea)
       and the aside scrolls independently; md sizes to content. -->
  <div
    class="{width} max-w-[92vw] {size === 'lg'
      ? 'h-[min(680px,86vh)]'
      : 'max-h-[86vh]'} flex flex-col"
  >
    <!-- pt-16 clears Modal's traffic-light/title row (~56px) so the first control
         doesn't slide under the header. -->
    <div class="flex flex-1 min-h-0 pt-16">
      <div class="flex-1 min-w-0 overflow-y-auto px-7 pb-6">
        {#if children}
          {@render children()}
        {/if}
      </div>
      {#if size === "lg" && aside}
        <div
          class="w-[300px] shrink-0 overflow-y-auto border-l border-[var(--hairline)] px-5 pb-6"
        >
          {@render aside()}
        </div>
      {/if}
    </div>

    <div
      class="flex items-center justify-between gap-4 border-t border-[var(--hairline)] px-5 py-3"
    >
      <div class="min-w-0 flex-1 truncate text-xs {error ? 'text-error' : 'text-base-content/50'}">
        {error || hint}
      </div>
      <div class="flex shrink-0 items-center gap-2.5">
        <Button variant="ghost" onclick={onClose} disabled={saving}>
          {cancelLabel || t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={onSubmit}
          disabled={saving || submitDisabled}
        >
          {submitLabel}
        </Button>
      </div>
    </div>
  </div>
</Modal>
