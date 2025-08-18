<script lang="ts">
  import { User } from '@lucide/svelte';

  interface Props {
    src?: string;      // 头像 URL
    letter?: string;   // 显示的字母
    size?: 'sm' | 'md' | 'lg';  // 尺寸
    class?: string;    // 额外的 CSS 类
  }

  let { src, letter, size = 'md', class: className = '' }: Props = $props();

  // 生成默认头像 SVG
  function generateLetterAvatar(letter?: string): string {
    const displayLetter = letter ? letter.charAt(0).toUpperCase() : '?';
    const color = letter ? '#6B7280' : '#9CA3AF';
    
    return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='16' fill='${encodeURIComponent(color)}'/%3E%3Ctext x='16' y='20' text-anchor='middle' fill='white' font-size='12' font-family='Arial'%3E${displayLetter}%3C/text%3E%3C/svg%3E`;
  }

  // 尺寸映射
  const sizeClasses = {
    sm: 'w-8 h-8',
    md: 'w-10 h-10',
    lg: 'w-12 h-12'
  };

  const iconSizes = {
    sm: 16,
    md: 20,
    lg: 24
  };

  const avatarSrc = $derived(src || (letter ? generateLetterAvatar(letter) : null));
  const sizeClass = $derived(sizeClasses[size]);
  const iconSize = $derived(iconSizes[size]);
</script>

<div class="relative {sizeClass} {className}">
  {#if avatarSrc}
    <img
      src={avatarSrc}
      alt="头像"
      class="w-full h-full rounded-full object-cover"
      onerror={() => {
        // 如果图片加载失败，可以在这里处理回退逻辑
        console.warn('Avatar image failed to load');
      }}
    />
  {:else}
    <!-- 默认头像图标 -->
    <div class="w-full h-full rounded-full bg-gray-200 flex items-center justify-center">
      <User size={iconSize} class="text-gray-600" />
    </div>
  {/if}
</div>
