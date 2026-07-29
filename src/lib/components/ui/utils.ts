import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Tailwind-aware className merge. clsx handles conditional / array / object
 * expansion; twMerge resolves conflicting Tailwind classes (last wins), so a
 * component's `class` prop reliably overrides internal variant classes instead
 * of stacking into two fighting rules.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
