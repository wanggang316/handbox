<script lang="ts">
  import { User, Edit } from "@lucide/svelte";
  import { proxyImage, shouldProxyImage } from "$lib/api/image";
  import { t } from "$lib/i18n";

  interface Props {
    src?: string;
    letter?: string;
    size?: "sm" | "md" | "lg";
    class?: string;
    editable?: boolean;
    onImageChange?: (file: File) => void;
  }

  let {
    src,
    letter,
    size = "md",
    class: className = "",
    editable = false,
    onImageChange,
  }: Props = $props();

  let fileInput = $state<HTMLInputElement>();

  let proxiedSrc = $state<string | null>(null);
  let isLoading = $state(false);
  let hasError = $state(false);

  const sizeClasses = {
    sm: "w-8 h-8",
    md: "w-12 h-12",
    lg: "w-16 h-16",
  };

  const iconSizes = {
    sm: 16,
    md: 24,
    lg: 32,
  };

  const avatarSrc = $derived(proxiedSrc || src || null);
  const sizeClass = $derived(sizeClasses[size]);
  const iconSize = $derived(iconSizes[size]);
  const fallbackLetter = $derived(letter ? letter.charAt(0).toUpperCase() : "");

  async function loadProxiedImage(url: string) {
    if (!shouldProxyImage(url)) {
      proxiedSrc = url;
      return;
    }

    isLoading = true;
    hasError = false;

    try {
      const dataUrl = await proxyImage(url);
      proxiedSrc = dataUrl;
    } catch (error) {
      console.error("Failed to load proxied image:", error);
      hasError = true;
      proxiedSrc = null;
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    if (src) {
      loadProxiedImage(src);
    } else {
      proxiedSrc = null;
      hasError = false;
    }
  });

  function handleFileUpload() {
    if (editable && fileInput) {
      fileInput.click();
    }
  }

  function handleFileChange(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (file && onImageChange) {
      onImageChange(file);
    }
  }
</script>

<div class="relative {sizeClass} {className} group">
  <button
    class="w-full h-full rounded-full overflow-hidden border-none p-0 bg-transparent"
    class:cursor-pointer={editable}
    class:cursor-default={!editable}
    onclick={editable ? handleFileUpload : undefined}
    disabled={!editable}
    title={editable ? t("ui.clickToUpload") : undefined}
  >
    {#if avatarSrc}
      <img
        src={avatarSrc}
        alt={t("ui.avatarAlt")}
        class="w-full h-full rounded-full object-cover"
        onerror={() => {
          console.warn("Avatar image failed to load");
        }}
      />
    {:else}
      <div
        class="w-full h-full rounded-full bg-base-300 flex items-center justify-center text-base-content/80 font-semibold"
      >
        {#if fallbackLetter}
          {fallbackLetter}
        {:else}
          <User size={iconSize} class="text-base-content/70" />
        {/if}
      </div>
    {/if}

    {#if editable}
      <div
        class="absolute inset-0 bg-base-content/0 group-hover:bg-base-content/30 transition-all duration-[var(--dur-base)] rounded-full flex items-center justify-center"
      >
        <div
          class="opacity-0 group-hover:opacity-100 transition-opacity duration-[var(--dur-base)] text-base-100 text-xs text-center"
        >
          {t("ui.clickToUpload")}
        </div>
      </div>
    {/if}
  </button>

  {#if editable}
    <input
      bind:this={fileInput}
      type="file"
      accept="image/*"
      class="hidden"
      onchange={handleFileChange}
    />
  {/if}
</div>
