<script lang="ts">
  import Avatar from '../ui/Avatar.svelte';

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

  $: displayName = user.isLoggedIn ? user.username || '用户' : '未登录';
  $: planText = user.isLoggedIn ? (user.isPro ? 'Pro Plan' : 'Free Plan') : '';
</script>

<div 
  class="flex items-center justify-center gap-3 p-2 cursor-pointer hover:bg-base-200 transition-colors rounded-lg"
  on:click={onUserClick}
  on:keydown={(e) => e.key === 'Enter' && onUserClick()}
  role="button"
  tabindex="0"
>
  <!-- 头像 -->
  <div class="flex flex-row gap-2 items-center">
    <Avatar 
      src={user.avatar}
      letter={user.username}
      size="sm"
    />
  </div>

  <!-- 用户信息 -->
  <div class="leading-[1.4] flex-1">
    <div class="text-[14px] text-base-content/80">{displayName}</div>
    <div class={`text-[12px] ${user.isLoggedIn ? 'text-base-content/60' : 'text-primary'}`}>{planText}</div>
  </div>
</div>
