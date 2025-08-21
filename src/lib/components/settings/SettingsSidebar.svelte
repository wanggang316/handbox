<script lang="ts">
  import { User, Palette, Brain, Zap, Keyboard, Info } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Menu from '$lib/components/ui/Menu.svelte';

  type Item = { id: string; title: string; icon: any; description?: string };
  
  export let items: Item[] = [
    { id: 'account', title: '账户', icon: User, description: '登录状态、用户资料管理' },
    { id: 'general', title: '通用', icon: Palette, description: '外观、语言、主题等' },
    { id: 'models', title: '模型', icon: Brain, description: '供应商与模型配置' },
    { id: 'mcp', title: 'MCP', icon: Zap, description: 'MCP 服务器管理' },
    { id: 'shortcuts', title: '快捷键', icon: Keyboard, description: '键盘快捷键' },
    { id: 'about', title: '关于', icon: Info, description: '版本与链接' },
  ];

  function isActive(id: string) {
    return $page.url.pathname.startsWith(`/settings/${id}`);
  }

  function navTo(id: string) {
    goto(`/settings/${id}`);
  }
</script>

<div class="h-full flex flex-col bg-bg-secondary p-0 pt-10 rounded-2xl overflow-hidden">
  <div class="flex-1 overflow-y-auto p-0">
    <Menu 
      title=""
      items={items.map(i => ({
        id: i.id,
        title: i.title,
        isActive: isActive(i.id),
        icon: i.icon
      }))}
      onItemClick={(item) => navTo(item.id)}
      containerClass="h-full"
    />
  </div>
  <slot name="footer" />
</div>


