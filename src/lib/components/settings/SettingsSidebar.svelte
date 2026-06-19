<script lang="ts">
  import { User, Palette, Brain, Zap, Sparkles, Keyboard, Info, MousePointerClick, LayoutGrid, Wrench } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Menu from '$lib/components/ui/Menu.svelte';
  import { t } from '$lib/i18n';
  import type { Snippet } from 'svelte';

  let { footer }: { footer?: Snippet } = $props();

  type Item = { id: string; title: string; icon: any, url: string, isActive?: boolean };

  const baseItems: Item[] = $derived([
    { id: 'account', title: t('settings.sidebar.account'), icon: User, url: '/settings/account' },
    { id: 'general', title: t('settings.sidebar.general'), icon: Palette, url: '/settings/general' },
    { id: 'quicktools', title: t('settings.sidebar.quicktools'), icon: MousePointerClick, url: '/settings/quicktools' },
    { id: 'models', title: t('settings.sidebar.models'), icon: Brain, url: '/settings/models' },
    { id: 'agent-tools', title: t('settings.sidebar.agentTools'), icon: Wrench, url: '/settings/agent-tools' },
    { id: 'mcp', title: 'MCP', icon: Zap, url: '/settings/mcp' },
    { id: 'skills', title: t('settings.sidebar.skills'), icon: Sparkles, url: '/settings/skills' },
    { id: 'components', title: t('settings.sidebar.components'), icon: LayoutGrid, url: '/settings/components' },
    { id: 'shortcuts', title: t('settings.sidebar.shortcuts'), icon: Keyboard, url: '/settings/shortcuts' },
    { id: 'about', title: t('settings.sidebar.about'), icon: Info, url: '/settings/about' },
  ]);

  const defaultItem = $derived(baseItems.find(i => i.id === 'account'));

  const currentItemId = $derived(
    baseItems.find(i => $page.url.pathname.startsWith(i.url))?.id || defaultItem?.id || ''
  );

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
  {@render footer?.()}
</div>
