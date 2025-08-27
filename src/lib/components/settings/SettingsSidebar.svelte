<script lang="ts">
  import { User, Palette, Brain, Zap, Keyboard, Info } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Menu from '$lib/components/ui/Menu.svelte';

  type Item = { id: string; title: string; icon: any, url: string, isActive?: boolean };
  
  let baseItems: Item[] = [
    { id: 'account', title: '账户', icon: User, url: '/settings/account' },
    { id: 'general', title: '通用', icon: Palette, url: '/settings/general' },
    { id: 'models', title: '模型', icon: Brain, url: '/settings/models' },
    { id: 'mcp', title: 'MCP', icon: Zap, url: '/settings/mcp' },
    { id: 'shortcuts', title: '快捷键', icon: Keyboard, url: '/settings/shortcuts' },
    { id: 'about', title: '关于', icon: Info, url: '/settings/about' },
  ];

  let defaultItem = baseItems.find(i => i.id === 'account');
  let currentItemId = defaultItem?.id || '';

  // $: items = baseItems.map(i => ({
  //   ...i,
  //   isActive: $page.url.pathname.startsWith(i.url)
  // }));


  function navTo(id: string) {
    currentItemId = id;
    goto(baseItems.find(i => i.id === id)?.url || defaultItem?.url || '/settings/account');
  }
</script>

<div class="h-full flex flex-col bg-bg-secondary p-0 pt-10 rounded-2xl overflow-hidden">
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


