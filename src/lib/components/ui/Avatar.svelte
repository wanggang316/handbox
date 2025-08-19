<script lang="ts">
  import { User, Edit } from '@lucide/svelte';

  interface Props {
    src?: string;      // 头像 URL
    letter?: string;   // 显示的字母
    size?: 'sm' | 'md' | 'lg';  // 尺寸
    class?: string;    // 额外的 CSS 类
    editable?: boolean; // 是否可编辑
    onImageChange?: (file: File) => void; // 图片变更回调
  }

  let { src, letter, size = 'md', class: className = '', editable = false, onImageChange }: Props = $props();

  // 文件上传引用
  let fileInput = $state<HTMLInputElement>();

  // 生成默认头像 SVG
  function generateLetterAvatar(letter?: string): string {
    const displayLetter = letter ? letter.charAt(0).toUpperCase() : '?';
    const color = letter ? '#6B7280' : '#9CA3AF';
    
    return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='16' fill='${encodeURIComponent(color)}'/%3E%3Ctext x='16' y='20' text-anchor='middle' fill='white' font-size='12' font-family='Arial'%3E${displayLetter}%3C/text%3E%3C/svg%3E`;
  }

  // 尺寸映射
  const sizeClasses = {
    sm: 'w-8 h-8',
    md: 'w-12 h-12',
    lg: 'w-16 h-16'
  };

  const iconSizes = {
    sm: 16,
    md: 24,
    lg: 32
  };

  const avatarSrc = $derived(src || (letter ? generateLetterAvatar(letter) : null));
  const sizeClass = $derived(sizeClasses[size]);
  const iconSize = $derived(iconSizes[size]);

  // 处理文件上传
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
  <!-- 头像容器 -->
  <button 
    class="w-full h-full rounded-full overflow-hidden border-none p-0 bg-transparent"
    class:cursor-pointer={editable}
    class:cursor-default={!editable}
    onclick={editable ? handleFileUpload : undefined}
    disabled={!editable}
    title={editable ? "点击上传" : undefined}
  >
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
    
    <!-- 编辑遮罩层 -->
    {#if editable}
      <div class="absolute inset-0 bg-black/0 group-hover:bg-black/30 transition-all duration-200 rounded-full flex items-center justify-center">
        <div class="opacity-0 group-hover:opacity-100 transition-opacity duration-200 text-white text-xs text-center">
          点击上传
        </div>
      </div>
    {/if}
  </button>

  <!-- 隐藏的文件上传输入框 -->
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
