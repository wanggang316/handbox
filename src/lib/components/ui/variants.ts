import { tv, type VariantProps } from 'tailwind-variants';

/**
 * 原子组件变体表：集中定义 variant / size → 语义色 utility 的映射，
 * 作为跨组件复用的样式真源，替代各组件散落的 scoped `<style>` 皮肤。
 *
 * 约定：颜色一律走 daisyUI 语义 token（`bg-primary` / `text-base-content` …），
 * 严禁裸色值（`bg-red-500` 之类）。`enabled:hover:*` 精确对应旧的
 * `:hover:not(:disabled)`，避免 disabled 态被 hover 覆盖。
 */
export const button = tv({
  base: 'inline-flex items-center gap-2 rounded-md font-medium cursor-pointer transition-[color,background-color,opacity] duration-[var(--dur-fast)] ease-[var(--ease-out)] disabled:opacity-60 disabled:cursor-not-allowed',
  variants: {
    variant: {
      primary: 'bg-primary text-primary-content enabled:hover:opacity-90',
      secondary:
        'bg-base-200 text-base-content border border-base-300 enabled:hover:opacity-90',
      gray: 'bg-base-200 text-base-content/80 border border-base-300 enabled:hover:bg-base-300 enabled:hover:text-base-content enabled:hover:opacity-100',
      danger: 'bg-error text-error-content enabled:hover:opacity-90',
      ghost:
        'bg-transparent text-base-content/80 border border-base-300 enabled:hover:bg-base-300 enabled:hover:text-base-content enabled:hover:opacity-100',
      clear:
        'bg-transparent text-base-content enabled:hover:bg-base-300 enabled:hover:text-base-content enabled:hover:opacity-100'
    },
    size: {
      sm: 'px-2 py-1 text-sm',
      md: 'px-4 py-2'
    }
  },
  defaultVariants: {
    variant: 'primary',
    size: 'md'
  }
});

export type ButtonVariants = VariantProps<typeof button>;
