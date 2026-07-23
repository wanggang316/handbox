<script lang="ts">
  import type { Snippet } from 'svelte';
  import { button, type ButtonVariants } from './variants';
  import { cn } from './utils';

  interface Props {
    variant?: ButtonVariants['variant'];
    size?: ButtonVariants['size'];
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    customClass?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    type = 'button',
    customClass = '',
    onclick,
    children
  }: Props = $props();
</script>

<button
  class={cn(button({ variant, size }), customClass)}
  data-slot="button"
  {type}
  {disabled}
  {onclick}
>
  {@render children?.()}
</button>
