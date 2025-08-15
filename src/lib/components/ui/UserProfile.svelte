<script lang="ts">
  import { User } from '@lucide/svelte';

  // 用户信息接口
  interface UserInfo {
    isLoggedIn: boolean;
    username?: string;
    avatar?: string;
    isPro?: boolean;
  }

  // 外部传入的用户信息
  export let user: UserInfo = {
    isLoggedIn: false
  };

  // 点击处理函数
  export let onUserClick: () => void = () => {};

  // 生成默认头像 SVG
  function generateDefaultAvatar(username?: string): string {
    const displayName = username ? username.charAt(0).toUpperCase() : '?';
    const color = username ? '#6B7280' : '#9CA3AF';
    
    return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='16' fill='${encodeURIComponent(color)}'/%3E%3Ctext x='16' y='20' text-anchor='middle' fill='white' font-size='12' font-family='Arial'%3E${displayName}%3C/text%3E%3C/svg%3E`;
  }

  $: avatarSrc = user.avatar || generateDefaultAvatar(user.username);
  $: displayName = user.isLoggedIn ? user.username || '用户' : '未登录';
  $: planText = user.isLoggedIn ? (user.isPro ? 'Pro Plan' : 'Free Plan') : '';
</script>

<div 
  class="flex items-center justify-center gap-3 p-2 cursor-pointer hover:bg-[#EDEDED] transition-colors rounded-lg"
  on:click={onUserClick}
  on:keydown={(e) => e.key === 'Enter' && onUserClick()}
  role="button"
  tabindex="0"
>
  <!-- 头像 -->
  <div class="">
    {#if user.isLoggedIn}
      <img
        src={avatarSrc}
        alt={displayName}
        class="w-8 h-8 rounded-full"
      />
    {:else}
      <div class="w-8 h-8 rounded-full bg-gray-200 flex items-center justify-center">
        <User size={16} class="text-gray-600" />
      </div>
    {/if}
  </div>

  <!-- 用户信息 -->
  <div class="leading-[1.4] flex-1">
    <div class="text-[14px] font-medium text-[#757575]">{displayName}</div>
    <div class="text-[12px] {user.isLoggedIn ? 'text-[#b3b3b3]' : 'text-blue-500'}">{planText}</div>
  </div>
</div>
