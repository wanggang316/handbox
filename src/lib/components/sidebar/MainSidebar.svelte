<script lang="ts">
  import { currentPage } from "$lib/stores/ui";
  import { get } from "svelte/store";
  import Menu from "$lib/components/ui/Menu.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import UserProfile from "$lib/components/ui/UserProfile.svelte";
  import { 
    Box, 
    Code, 
    Gamepad2, 
    TrendingUp, 
    Image,
    BookOpen,
    Plus,
    Search
  } from '@lucide/svelte';

  const sessions = [
    { 
      id: "2", 
      title: "Claude Code 使用指南", 
      isActive: false
    },
    { 
      id: "3", 
      title: "经典贪食蛇网页游戏", 
      isActive: false
    },
    { id: "4", title: "Python npx 命令行工具介绍", isActive: false },
    { id: "5", title: "今日 AI 新闻热点汇总", isActive: true },
    { 
      id: "6", 
      title: "推荐股票学习资料", 
      isActive: false
    },
    { 
      id: "7", 
      title: "Go 语言学习资料推荐", 
      isActive: false
    },
    { 
      id: "8", 
      title: "小猫照片编辑生成", 
      isActive: false
    },
    { 
      id: "2", 
      title: "Claude Code 使用指南", 
      isActive: false
    },
    { 
      id: "3", 
      title: "经典贪食蛇网页游戏", 
      isActive: false
    },
    { id: "4", title: "Python npx 命令行工具介绍", isActive: false },
    { id: "5", title: "今日 AI 新闻热点汇总", isActive: true },
    { 
      id: "6", 
      title: "推荐股票学习资料", 
      isActive: false
    },
    { 
      id: "7", 
      title: "Go 语言学习资料推荐", 
      isActive: false
    },
    { 
      id: "8", 
      title: "小猫照片编辑生成", 
      isActive: false
    },
  ];

  function go(page: "chat" | "artifact") {
    currentPage.set(page);
  }

  function handleSessionClick(session: any) {
    console.log('Clicked session:', session);
  }

  function handleArtifactClick(title: string) {
    console.log('Clicked artifact menu');
  }

  function handleNewChatClick() {
    console.log('Clicked new chat');
    // 这里可以添加新建聊天的逻辑
  }

  function handleSearchClick() {
    console.log('Clicked search');
    // 这里可以添加搜索的逻辑
  }

  function handleUserClick() {
    if (currentUser.isLoggedIn) {
      console.log('打开用户设置');
      // 这里可以添加打开用户设置页面的逻辑
    } else {
      console.log('跳转到登录页面');
      // 这里可以添加跳转到登录页面的逻辑
    }
  }

  // 模拟用户状态，实际应该从 store 或 API 获取
  // 可以切换这两个状态来测试不同的显示效果
  const currentUser = {
    isLoggedIn: true,
    username: "Alex",
    avatar: undefined, // 使用默认头像
    isPro: true
  };

  // 未登录状态示例：
  // const currentUser = {
  //   isLoggedIn: false
  // };

  $: active = $currentPage as "chat" | "artifact";
</script>

<div class="h-full flex flex-col bg-[#f8f8f8] p-0 pt-15">
  <!-- 顶部固定区域 -->
  <div class="flex-shrink-0 space-y-6 mb-6">
    <!-- 顶部操作 -->
    <div class="flex gap-2 px-4">
      <RoundButton
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
      onClick={() => handleArtifactClick("Artifact")}
    />
    </div>
    
  </div>

  <!-- 中间可滚动区域 -->
  <div class="flex-1 min-h-0">
    <Menu 
      title="聊天"
      items={sessions} 
      onItemClick={handleSessionClick}
      containerClass="h-full"
    />
  </div>

  <!-- 用户信息 -->
   <div class="flex-shrink-0 p-2">
    <UserProfile user={currentUser} onUserClick={handleUserClick} />
   </div>
  
</div>
