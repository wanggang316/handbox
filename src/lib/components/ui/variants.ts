import { tv, type VariantProps } from 'tailwind-variants';

/**
 * Atomic component variant tables: the central variant / size -> semantic-color
 * utility mapping, serving as the single style source shared across components
 * (no per-component scoped `<style>` skins).
 *
 * Conventions: colors use daisyUI semantic tokens only (`bg-primary` /
 * `text-base-content` ...), never raw palette values (`bg-red-500`).
 * `enabled:hover:*` matches `:hover:not(:disabled)` exactly, so the disabled
 * state is never restyled by hover.
 */
/**
 * Single button variant table on three axes: `variant x size x shape`.
 * Icon buttons = `size: 'icon*'`; circular = `size:'icon' shape:'pill'`;
 * pill = `shape:'pill'`. The `border border-transparent` baseline keeps
 * bordered and borderless variants the same height.
 */
export const button = tv({
  base: 'inline-flex shrink-0 items-center justify-center gap-1.5 border border-transparent font-medium whitespace-nowrap cursor-pointer select-none outline-none transition-[color,background-color,border-color,opacity] duration-[var(--dur-fast)] ease-[var(--ease-out)] focus-visible:ring-2 focus-visible:ring-primary/50 disabled:pointer-events-none disabled:opacity-60 aria-invalid:border-error',
  variants: {
    variant: {
      primary: 'bg-primary text-primary-content enabled:hover:bg-primary/90',
      secondary: 'bg-base-200 text-base-content border-base-300 enabled:hover:bg-base-300',
      gray: 'bg-base-200 text-base-content/80 border-base-300 enabled:hover:bg-base-300 enabled:hover:text-base-content',
      danger: 'bg-error text-error-content enabled:hover:bg-error/90',
      accent: 'bg-accent text-accent-content enabled:hover:bg-accent/90',
      neutral: 'bg-neutral text-neutral-content enabled:hover:bg-neutral/90',
      ghost: 'bg-transparent text-base-content border-base-300 enabled:hover:bg-base-300',
      clear: 'bg-transparent text-base-content enabled:hover:bg-base-300',
      link: 'border-transparent text-primary underline-offset-4 enabled:hover:underline'
    },
    size: {
      sm: 'h-7 gap-1 px-2.5 text-xs',
      md: 'h-8 px-3.5 text-sm',
      lg: 'h-10 px-5 text-base',
      icon: 'size-8 p-0',
      'icon-sm': 'size-7 p-0',
      'icon-lg': 'size-10 p-0'
    },
    shape: {
      default: 'rounded-md',
      pill: 'rounded-full'
    }
  },
  defaultVariants: {
    variant: 'primary',
    size: 'md',
    shape: 'default'
  }
});

export type ButtonVariants = VariantProps<typeof button>;
