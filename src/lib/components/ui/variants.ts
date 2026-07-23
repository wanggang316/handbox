import { tv, type VariantProps } from 'tailwind-variants';

/**
 * 原子组件变体表：集中定义 variant / size → 语义色 utility 的映射，
 * 作为跨组件复用的样式真源，替代各组件散落的 scoped `<style>` 皮肤。
 *
 * 约定：颜色一律走 daisyUI 语义 token（`bg-primary` / `text-base-content` …），
 * 严禁裸色值（`bg-red-500` 之类）。`enabled:hover:*` 精确对应旧的
 * `:hover:not(:disabled)`，避免 disabled 态被 hover 覆盖。
 */
/**
 * 单一按钮变体表（吸收原 IconButton / CircleButton / RoundButton / ArrowButton）：
 * `variant × size × shape` 三轴。图标按钮 = `size: 'icon*'`；圆形 = `size:'icon' shape:'pill'`；
 * 药丸 = `shape:'pill'`。`border border-transparent` 基线让有边框变体与无边框变体等高。
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

/**
 * 方形图标按钮（icon-only）。variant 仅 `ghost`（透明底、base-300 hover）。
 * `size` / `rounded` 仍由调用方以字符串 prop 传入，经 `cn()` 合并覆盖。
 */
export const iconButton = tv({
  base: 'flex items-center justify-center transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] disabled:opacity-60 disabled:cursor-not-allowed',
  variants: {
    variant: {
      ghost: 'bg-transparent text-base-content enabled:hover:bg-base-300'
    }
  },
  defaultVariants: {
    variant: 'ghost'
  }
});

export type IconButtonVariants = VariantProps<typeof iconButton>;

/**
 * 圆形图标按钮（FAB 风）。`neutral` = 实心中性填充，`secondary` = surface 提升。
 */
export const circleButton = tv({
  base: 'flex items-center justify-center transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] disabled:opacity-60 disabled:cursor-not-allowed',
  variants: {
    variant: {
      neutral: 'bg-neutral text-neutral-content enabled:hover:bg-neutral/90',
      secondary: 'bg-base-200 text-base-content enabled:hover:bg-base-300'
    }
  },
  defaultVariants: {
    variant: 'neutral'
  }
});

export type CircleButtonVariants = VariantProps<typeof circleButton>;

/**
 * 药丸按钮（带 label / loading）。disabled 或 loading 态统一压成 base-300 底、
 * 降透明度并禁交互——组件把 `disabled={disabled || loading}` 传给原生属性。
 */
export const roundButton = tv({
  base: 'flex items-center justify-center gap-1.5 transition duration-[var(--dur-fast)] ease-[var(--ease-out)] disabled:bg-base-300 disabled:opacity-60 disabled:cursor-not-allowed disabled:pointer-events-none',
  variants: {
    variant: {
      primary: 'bg-primary text-primary-content enabled:hover:opacity-90',
      accent: 'bg-accent text-accent-content enabled:hover:bg-accent/90',
      danger: 'bg-error text-error-content enabled:hover:bg-error/90',
      secondary: 'bg-base-300 text-base-content/80 enabled:hover:bg-base-300/80'
    }
  },
  defaultVariants: {
    variant: 'primary'
  }
});

export type RoundButtonVariants = VariantProps<typeof roundButton>;
