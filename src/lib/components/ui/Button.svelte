<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Loader } from '@lucide/svelte';
  import { button, type ButtonVariants } from './variants';
  import { cn } from './utils';

  interface Props {
    variant?: ButtonVariants['variant'];
    size?: ButtonVariants['size'];
    shape?: ButtonVariants['shape'];
    disabled?: boolean;
    loading?: boolean;
    type?: 'button' | 'submit' | 'reset';
    /** Preferred styling hook. */
    class?: string;
    /** Legacy alias for `class`, kept for existing call sites. */
    customClass?: string;
    title?: string;
    ariaLabel?: string;
    onclick?: (event: MouseEvent) => void;
    /** Receives the underlying <button> element (to focus, measure, or anchor to it). */
    elementRef?: (el: HTMLButtonElement | null) => void;
    children?: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    shape = 'default',
    disabled = false,
    loading = false,
    type = 'button',
    class: className = '',
    customClass = '',
    title = '',
    ariaLabel = '',
    onclick,
    elementRef,
    children
  }: Props = $props();

  let el: HTMLButtonElement | null = null;
  $effect(() => {
    elementRef?.(el);
  });

  const inactive = $derived(disabled || loading);

  function handleClick(event: MouseEvent) {
    if (!inactive) onclick?.(event);
  }
</script>

<button
  bind:this={el}
  class={cn(button({ variant, size, shape }), className, customClass)}
  data-slot="button"
  {type}
  disabled={inactive}
  title={title || undefined}
  aria-label={ariaLabel || undefined}
  aria-busy={loading || undefined}
  onclick={handleClick}
>
  {#if loading}
    <Loader class="size-4 animate-spin" />
  {/if}
  {@render children?.()}
</button>
