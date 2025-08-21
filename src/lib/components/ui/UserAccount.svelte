<script lang="ts">
  import Avatar from './Avatar.svelte';

  // 用户信息接口
  interface UserInfo {
    isLoggedIn: boolean;
    username?: string;
    email?: string;
    avatar?: string;
    isPro?: boolean;
  }

  interface Props {
    user: UserInfo;
  }

  let { user }: Props = $props();

  const displayName = $derived(user.isLoggedIn ? user.username || '用户' : '未登录');
  const planText = $derived(user.isLoggedIn ? (user.isPro ? 'Pro' : 'Free') : '');
</script>

{#if user.isLoggedIn}
  <!-- 已登录状态的用户信息显示 -->
  <div class="flex flex-row gap-2 items-center">
    <Avatar 
      src={user.avatar}
      letter={user.username}
      size="md"
    />
    <div class="flex-1 flex flex-col">
      <div class="flex flex-row gap-x-2 items-center">
        <div class="text-md text-[#757575]">{displayName}</div>
        {#if planText}
          <div class="text-xs text-blue-500 rounded-xl bg-blue-100 border border-blue-500 px-2 py-0">
            {planText}
          </div>
        {/if}
      </div>
      
      {#if user.email}
        <div class="text-[12px] text-[#b3b3b3]">{user.email}</div>
      {/if}
    </div>
  </div>
{:else}
  <!-- 未登录状态的用户信息显示 -->
  <div class="flex flex-row gap-2 items-center">
    <Avatar size="lg" />
    <div class="flex-1">
      <div class="text-xs text-[#757575]">未登录</div>
    </div>
  </div>
{/if}
