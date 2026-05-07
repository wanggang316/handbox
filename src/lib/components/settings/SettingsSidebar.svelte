<script lang="ts">
  import { User, Palette, Brain, BookOpen, Zap, Keyboard, Info, MousePointerClick, LayoutGrid } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Menu from '$lib/components/ui/Menu.svelte';

  type Item = { id: string; title: string; icon: any, url: string, isActive?: boolean };

  let baseItems: Item[] = [
    { id: 'account', title: '账户', icon: User, url: '/settings/account' },
    { id: 'general', title: '通用', icon: Palette, url: '/settings/general' },
    { id: 'quicktools', title: '快捷工具', icon: MousePointerClick, url: '/settings/quicktools' },
    { id: 'models', title: '模型', icon: Brain, url: '/settings/models' },
    { id: 'words', title: '单词本', icon: BookOpen, url: '/settings/words' },
    { id: 'mcp', title: 'MCP', icon: Zap, url: '/settings/mcp' },
    { id: 'components', title: '组件', icon: LayoutGrid, url: '/settings/components' },
    { id: 'shortcuts', title: '快捷键', icon: Keyboard, url: '/settings/shortcuts' },
    { id: 'about', title: '关于', icon: Info, url: '/settings/about' },
  ];

  let defaultItem = baseItems.find(i => i.id === 'account');
  
  $: currentItemId = baseItems.find(i => $page.url.pathname.startsWith(i.url))?.id || defaultItem?.id || '';

  function navTo(id: string) {
    goto(baseItems.find(i => i.id === id)?.url || defaultItem?.url || '/settings/account');
  }
</script>

<div class="h-full flex flex-col p-0 pt-10 overflow-hidden">
  <div class="flex-1 overflow-y-auto p-0">
    <Menu 
      title=""
      items={baseItems}
      onItemClick={(item) => navTo(item.id)}
      containerClass="h-full"
      activeId={currentItemId}
    />
  </div>
  <slot name="footer" />
</div>
