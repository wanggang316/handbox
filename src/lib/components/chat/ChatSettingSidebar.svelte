<script lang="ts">
  import Menu from '../ui/Menu.svelte';
  import { MessageSquare, Settings, Server } from '@lucide/svelte';

  interface Props {
    activeTab: string;
    onTabChange: (tab: string) => void;
  }

  let { activeTab, onTabChange }: Props = $props();

  const menuItems = [
    {
      id: 'prompt',
      title: '提示词',
      icon: MessageSquare,
      isActive: activeTab === 'prompt'
    },
    {
      id: 'model',
      title: '模型参数',
      icon: Settings,
      isActive: activeTab === 'model'
    },
    {
      id: 'mcp',
      title: 'MCP 服务器',
      icon: Server,
      isActive: activeTab === 'mcp'
    }
  ];

  function handleItemClick(item: any) {
    onTabChange(item.id);
  }
</script>

<div class="w-36 bg-[#f8f8f8] m-2 rounded-2xl overflow-hidden">
  <Menu 
    title="聊天设置"
    items={menuItems}
    onItemClick={handleItemClick}
    containerClass="h-full pt-10"
  />
</div>
