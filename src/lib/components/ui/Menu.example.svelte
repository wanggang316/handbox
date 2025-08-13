<!-- Menu 组件使用示例 -->
<script lang="ts">
  import Menu from "./Menu.svelte";
  import { 
    Home, 
    Settings, 
    User, 
    MessageCircle, 
    Heart,
    Star,
    Code,
    Gamepad2
  } from '@lucide/svelte';
  
  const menuItems = [
    {
      id: "1",
      title: "首页",
      icon: Home,
      isActive: true
    },
    {
      id: "2", 
      title: "设置",
      icon: Settings,
      isActive: false
    },
    {
      id: "3",
      title: "用户资料", 
      icon: User,
      iconPosition: "right" as const,
      isActive: false
    },
    {
      id: "4",
      title: "消息中心",
      icon: MessageCircle,
      isActive: false
    }
  ];
  
  const projectItems = [
    {
      id: "p1",
      title: "代码项目",
      icon: Code,
      isActive: false
    },
    {
      id: "p2",
      title: "游戏开发",
      icon: Gamepad2,
      isActive: true
    },
    {
      id: "p3",
      title: "收藏项目",
      icon: Heart,
      isActive: false
    }
  ];
  
  const simpleItems = [
    { id: "s1", title: "无图标项目 1", isActive: false },
    { id: "s2", title: "无图标项目 2", isActive: true },
    { id: "s3", title: "无图标项目 3", isActive: false }
  ];
  
  function handleItemClick(item: any) {
    console.log('点击了菜单项:', item);
  }
</script>

<div class="p-6 space-y-6 max-w-md bg-gray-50">
  <h2 class="text-lg font-semibold mb-4">Menu 组件使用示例</h2>
  
  <!-- 带图标的菜单 -->
  <div class="bg-white p-4 rounded-lg shadow-sm">
    <Menu
      title="导航菜单"
      items={menuItems}
      onItemClick={handleItemClick}
    />
  </div>
  
  <!-- 项目菜单 -->
  <div class="bg-white p-4 rounded-lg shadow-sm">
    <Menu
      title="我的项目"
      items={projectItems}
      onItemClick={handleItemClick}
    />
  </div>
  
  <!-- 简单文本菜单 -->
  <div class="bg-white p-4 rounded-lg shadow-sm">
    <Menu
      title="简单菜单"
      items={simpleItems}
      onItemClick={handleItemClick}
    />
  </div>
</div>
