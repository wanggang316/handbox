import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Tailwind-aware className 合并。
 * clsx 负责条件 / 数组 / 对象展开，twMerge 消解冲突的 Tailwind 类（后者胜），
 * 使组件的 `class` prop 能可靠覆盖内部变体类，而非叠加成互相打架的两条规则。
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
