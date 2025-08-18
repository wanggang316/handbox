<script lang="ts">
  import Button from '$lib/components/ui/Button.svelte';
import TableGroup from '$lib/components/ui/table/TableGroup.svelte';
import { User } from '@lucide/svelte';

  // 示例用户数据
  let user = {
    isLoggedIn: false,
    username: '',
    email: 'wanggang@gmail.com',
    avatar: '', // 将使用默认头像
    isPro: true
  };

  // 生成默认头像 SVG
  function generateDefaultAvatar(username?: string): string {
    const displayName = username ? username.charAt(0).toUpperCase() : '?';
    const color = username ? '#6B7280' : '#9CA3AF';
    
    return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='16' fill='${encodeURIComponent(color)}'/%3E%3Ctext x='16' y='20' text-anchor='middle' fill='white' font-size='12' font-family='Arial'%3E${displayName}%3C/text%3E%3C/svg%3E`;
  }

  $: avatarSrc = user.avatar || generateDefaultAvatar(user.username);
  $: displayName = user.isLoggedIn ? user.username || '用户' : '未登录';
  $: planText = user.isLoggedIn ? (user.isPro ? 'Pro' : 'Free') : '';


  function handleUserClick() {
    console.log('用户点击了用户资料');
    // 这里可以添加用户点击的处理逻辑，比如打开用户资料编辑对话框
  }
</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">

  {#if user.isLoggedIn}
  <TableGroup>
    <div class="flex flex-row gap-2 items-center justify-center px-6 py-6">
      <img
        src={avatarSrc}
        alt={displayName}
        class="w-12 h-12 rounded-full"
      />
      <div class="flex-1 flex flex-col">
        <div class="flex flex-row gap-x-2 items-center">
          <div class="text-md text-[#757575]">{displayName}</div>
          <div class="text-xs text-blue-500 rounded-xl bg-blue-100 border border-blue-500 px-2 py-0">{planText}</div>
        </div>
        
        <div class="text-[12px] {user.isLoggedIn ? 'text-[#b3b3b3]' : 'text-blue-500'}">{user.email}</div>
      </div>

      <Button variant="gray" size="sm">
        编辑资料
      </Button>
    </div>
    
  </TableGroup>
  <div>
    <Button variant="gray" size="sm">
      退出登录
    </Button>
  </div>
  {:else}
  <TableGroup>
    <div class="flex flex-row gap-2 items-center px-6 py-6">
      <div
        class="w-12 h-12 rounded-full bg-bg-hover flex items-center justify-center"
      >
      <User />
      </div>
      <div class="flex-1">
        <div class="text-xs text-[#757575]">未登录</div>
      </div>
      <Button variant="gray" size="sm">
        登录
      </Button>
    </div>
    
  </TableGroup>
  {/if}
</div>


