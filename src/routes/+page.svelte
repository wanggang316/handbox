<script lang="ts">
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import { settingsState } from "$lib/states/settings.svelte";
  import { t } from "$lib/i18n";

  // Hold the splash until first-screen data is ready, not for a fixed duration.
  // MIN_VISIBLE avoids a jarring flash; MAX_WAIT caps the wait if an IPC stalls.
  const MIN_VISIBLE = 400;
  const MAX_WAIT = 3000;

  onMount(() => {
    if (!browser) return;

    // The main window starts visible:false: show it only after the splash's
    // first frame has painted (double rAF) to avoid startup flashes. The Rust
    // side has a 4s fallback show.
    if (isTauriEnvironment()) {
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          const w = getCurrentWindow();
          w.show()
            .then(() => w.setFocus())
            .catch((e) => console.error("Failed to show main window:", e));
        }),
      );
    }

    let entered = false;
    const enter = () => {
      if (entered) return;
      entered = true;
      // replaceState: keep the splash out of history so back never returns here
      goto("/agent", { replaceState: true });
    };

    const delay = (ms: number) =>
      new Promise((resolve) => setTimeout(resolve, ms));

    // Wait only for settings (theme/language — avoids a theme flash after entry).
    // Providers/models preload in the root layout without blocking entry.
    // allSettled: load failures must not block entry either.
    const ready = Promise.allSettled([settingsState.loadSettings()]);

    // Enter when ready (after at least MIN_VISIBLE), or when MAX_WAIT elapses.
    Promise.race([Promise.all([ready, delay(MIN_VISIBLE)]), delay(MAX_WAIT)]).then(
      enter,
    );
  });
</script>

<div class="splash">
  <div class="splash__content">
    <div class="splash__brand">
      <h1 class="splash__title">HandBox</h1>
    </div>
    <div class="splash__loader" role="status" aria-label={t("ui.loading")}>
      <span></span><span></span><span></span>
    </div>
  </div>
</div>

<style>
  .splash {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    /* canvas background; follows data-theme light/dark */
    background-color: var(--base-100);
    color: var(--base-content);
  }

  .splash__content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.25rem;
    animation: splash-fade-in var(--dur-slow, 300ms) var(--ease-out, ease-out)
      both;
  }

  .splash__brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.375rem;
  }

  .splash__title {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--base-content);
  }

  .splash__loader {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .splash__loader span {
    width: 7px;
    height: 7px;
    border-radius: 9999px;
    background-color: var(--primary);
    animation: splash-dot 1.2s var(--ease-standard, ease-in-out) infinite;
  }

  .splash__loader span:nth-child(2) {
    animation-delay: 0.16s;
  }

  .splash__loader span:nth-child(3) {
    animation-delay: 0.32s;
  }

  @keyframes splash-fade-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes splash-dot {
    0%,
    100% {
      opacity: 0.25;
      transform: scale(0.8);
    }
    50% {
      opacity: 1;
      transform: scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .splash__content,
    .splash__loader span {
      animation: none;
    }
  }
</style>
