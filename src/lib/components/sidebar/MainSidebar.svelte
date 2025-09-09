<script lang="ts">
  import { chatState } from "$lib/states/chat.svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import Menu from "$lib/components/ui/Menu.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import UserSidebar from "$lib/components/sidebar/UserSidebar.svelte";
  import { 
    Box, 
    Plus,
    Search
  } from '@lucide/svelte';
  import { openSettingsWindow } from '$lib/api/window';

  // 获取当前选中的聊天 ID
  let currentChatId = $derived(
    browser && $page.url ? $page.url.searchParams.get('id') || '' : ''
  );

  // 将真实聊天数据转换为 Menu 组件期望的格式
  let chats = $derived(
    chatState.chats.map(chat => ({
      id: chat.id,
      title: chat.name
    }))
  );

  function handleChatClick(chat: any) {
    console.log('Clicked chat:', chat);
    // 使用 SvelteKit 的客户端路由导航，避免页面重新加载
    goto(`/chat?id=${chat.id}`);
  }

  function handleArtifactClick() {
    console.log('Clicked artifact menu');
    goto(`/artifacts`);
  }

  function handleNewChatClick() {
    console.log('Clicked new chat');
    // 导航到聊天页面，不带 id 参数（表示新建聊天）
    goto('/chat');
  }

  function handleSearchClick() {
    console.log('Clicked search');
    // 导航到搜索页面
    goto('/search');
  }

  function handleUserClick() {
    if (currentUser.isLoggedIn) {
      console.log('打开用户设置');
      // 打开独立的设置窗口
      openSettingsWindow().catch(err => {
        console.error('Failed to open settings window:', err);
      });
    } else {
      console.log('跳转到登录页面');
      // 这里可以添加跳转到登录页面的逻辑
    }
  }

  // 模拟用户状态，实际应该从 store 或 API 获取
  // 可以切换这两个状态来测试不同的显示效果
  let currentUser = $state({
    isLoggedIn: true,
    username: "Alex",
    avatar: "https://lh3.googleusercontent.com/a/ACg8ocKdKLfYXuyg3WFnA4HGTrga_E2YtSw_r9x3079cyaNFsHSwsYAh=s96-c", // 使用默认头像
    isPro: true
  });

  // 未登录状态示例：
  // let currentUser = $state({
  //   isLoggedIn: false
  // });

</script>

<div class="h-full flex flex-col bg-bg-secondary p-0 pt-15 rounded-2xl overflow-hidden">
  <!-- 顶部固定区域 -->
  <div class="flex-shrink-0 space-y-6 mb-6">
    <!-- 顶部操作 -->
    <div class="flex gap-2 px-4">
      <RoundButton
        customClass="flex-1"
        label="New chat"
        icon={Plus}
        on:click={handleNewChatClick}
      />
      <CircleButton
        icon={Search}
        ariaLabel="搜索"
        on:click={handleSearchClick}
      />
    </div>
    
    <div class="flex px-2">
      <MenuButton
      title="Artifacts"
      icon={Box}
      iconSize={20}
      onClick={() => handleArtifactClick()}
    />
    </div>
    
  </div>

  <!-- 中间可滚动区域 -->
  <div class="flex-1 min-h-0">
    <Menu 
      title="聊天"
      items={chats} 
      onItemClick={handleChatClick}
      containerClass="h-full"
      activeId={currentChatId}
    />
  </div>

  <!-- 用户信息 -->
   <div class="flex-shrink-0 p-2">
    <UserSidebar user={currentUser} onUserClick={handleUserClick} />
   </div>
  
</div>
