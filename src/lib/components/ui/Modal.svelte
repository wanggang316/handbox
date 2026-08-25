<script lang="ts">
  import { Dialog } from "bits-ui";
  import TrafficLightsRedButton from "./TrafficLightsRedButton.svelte";
  import TitleBar from "./TitleBar.svelte";

  interface Props {
    open?: boolean;
    title?: string;
    showCloseButton?: boolean;
    closeOnBackdropClick?: boolean;
    onClose?: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    showCloseButton = true,
    closeOnBackdropClick = false,
    onClose = () => {},
    children,
  }: Props = $props();

  // All close paths (Escape / outside click / traffic light / programmatic) converge
  // to open=false via bind:open; fire onClose once on the true -> false transition.
  let wasOpen = false;
  $effect(() => {
    if (wasOpen && !open) onClose();
    wasOpen = open;
  });

  // Programmatic close for callers holding a bind:this reference.
  export function handleClose() {
    open = false;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay
      class="dlg-overlay fixed inset-0"
      style="z-index: var(--z-overlay); background-color: var(--overlay);"
    />

    <!-- bits-ui Portal mounts bare children to <body> immediately (only Overlay/Content
         have presence gating), so TitleBar needs {#if open} — otherwise a closed Modal
         would still inject this fixed top drag strip. While open it sits above the
         backdrop (z between --z-overlay/--z-modal) so the window stays draggable. -->
    {#if open}
      <div style="position: relative; z-index: 10055;">
        <TitleBar showToggleButton={false} />
      </div>
    {/if}

    <!-- Centering and enter/exit animation both use transform. Tailwind's
         -translate-x/y-1/2 is avoided: in v4 it sets the separate translate
         property, which stacks with the keyframe transform. -->
    <!-- No autofocus on the first focusable element: with no preceding mouse
         interaction (a dialog shown at startup) autofocus counts as
         :focus-visible and the first button gets a ring out of nowhere. -->
    <Dialog.Content
      interactOutsideBehavior={closeOnBackdropClick ? "close" : "ignore"}
      onOpenAutoFocus={(e) => e.preventDefault()}
      class="dlg-content fixed left-1/2 top-1/2 max-h-[90vh] max-w-[90vw] rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] shadow-2xl outline-none"
      style="z-index: var(--z-modal); transform: translate(-50%, -50%);"
    >
      {#if showCloseButton || title}
        <div class="absolute left-0 top-0 z-10 flex items-center px-5 py-4">
          {#if showCloseButton}
            <TrafficLightsRedButton onClick={() => (open = false)} />
          {/if}
          <Dialog.Title
            class={title
              ? "ml-4 text-base font-medium text-base-content/80"
              : "sr-only"}
          >
            {title || "对话框"}
          </Dialog.Title>
        </div>
      {:else}
        <Dialog.Title class="sr-only">对话框</Dialog.Title>
      {/if}

      <!-- No overflow-hidden here: it would clip inner dropdowns / Select. -->
      {#if children}
        {@render children()}
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
