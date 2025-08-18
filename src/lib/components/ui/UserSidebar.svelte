<script lang="ts">
  import Avatar from './Avatar.svelte';

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
  class="flex items-center justify-center gap-3 p-2 cursor-pointer hover:bg-[#EDEDED] transition-colors rounded-lg"
  on:click={onUserClick}
  on:keydown={(e) => e.key === 'Enter' && onUserClick()}
  role="button"
  tabindex="0"
>
  <!-- 头像 -->
  <div class="">
    <Avatar 
      src={user.avatar}
      letter={user.username}
      size="sm"
    />
  </div>

  <!-- 用户信息 -->
  <div class="leading-[1.4] flex-1">
    <div class="text-[14px] text-[#757575]">{displayName}</div>
    <div class="text-[12px] {user.isLoggedIn ? 'text-[#b3b3b3]' : 'text-blue-500'}">{planText}</div>
  </div>
</div>
