<script lang="ts">
  import { currentPage } from "$lib/stores/ui";
  import { get } from "svelte/store";
  import Menu from "$lib/components/ui/Menu.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import { 
    Box, 
    Code, 
    Gamepad2, 
    TrendingUp, 
    Image,
    BookOpen
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

  $: active = $currentPage as "chat" | "artifact";
</script>

<div class="h-full bg-[#f8f8f8] p-4 pt-10 flex flex-col">
  <!-- 顶部操作 -->
  <div class="flex gap-2 mb-4">
    <button
      class="flex-1 h-9 bg-[#5661f6] hover:opacity-90 text-white rounded-full text-[16px] flex items-center justify-center gap-1.5 transition-colors"
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <path d="M12 5v14m-7-7h14" />
      </svg>
      New chat
    </button>
    <button
      class="w-9 h-9 bg-black hover:bg-black/80 text-white rounded-full flex items-center justify-center transition-colors"
      aria-label="搜索"
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <circle cx="11" cy="11" r="8" />
        <path d="m21 21-4.35-4.35" />
      </svg>
    </button>
  </div>
  
  <MenuButton
    buttonClass="my-6"
    title="Artifact"
    icon={Box}
    iconSize={16}
    onClick={() => handleArtifactClick("Artifact")}
  />
  

  <Menu 
    title="聊天"
    items={sessions} 
    onItemClick={handleSessionClick}
  />

  <!-- 用户信息 -->
  <div class="pt-3 mt-3 border-t border-[#e7e7e7] flex items-center gap-3">
    <img
      src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='16' fill='%236B7280'/%3E%3Ctext x='16' y='20' text-anchor='middle' fill='white' font-size='12' font-family='Arial'%3EAlex%3C/text%3E%3C/svg%3E"
      alt="Alex"
      class="w-8 h-8 rounded-full"
    />
    <div class="leading-[1.4]">
      <div class="text-[16px] font-semibold text-[#757575]">Alex</div>
      <div class="text-[16px] text-[#b3b3b3]">Pro Plan</div>
    </div>
  </div>
</div>
