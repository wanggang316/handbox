<script lang="ts">
  import SearchPage from '$lib/components/search/SearchPage.svelte';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { searchState } from '$lib/states/search.svelte';
  import { chatActions } from '$lib/states/chat.svelte';

let searchQuery = $state('');
let selectedFilter = $state('all');

// 过滤选项
const filterOptions = [
  { value: 'all', label: 'All' },
  { value: 'messages', label: 'Messages' },
  { value: 'sessions', label: 'Sessions' },
  { value: 'artifacts', label: 'Artifacts' }
];

// 执行搜索
async function performSearch() {
  if (!searchQuery.trim()) return;
  try {
    const types = selectedFilter === 'all'
      ? undefined
      : [selectedFilter === 'messages' ? 'message' : selectedFilter.slice(0, -1) as any];
    await searchState.search({ query: searchQuery, types });
  } catch (error) {
    console.error('Search failed:', error);
  }
}

// 处理搜索输入
function handleSearchInput() {
  if (searchQuery.trim()) {
    performSearch();
  } else {
    searchState.clearResults();
  }
}

// 处理键盘事件
function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    performSearch();
  }
}

// 跳转到聊天
function goToChat(chatId: string) {
  chatActions.switchToChat(chatId);
  goto(`/chat?id=${chatId}`);
}

// 获取搜索结果图标
function getResultIcon(type: string): string {
  switch (type) {
    case 'message':
      return 'M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z';
    case 'session':
      return 'M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z';
    case 'artifact':
      return 'M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547A1.934 1.934 0 004 17.693v3.621l2.053-.410a6 6 0 013.86-.517l.318.158a6 6 0 003.86.517L16.947 21v-3.621c0-.987.428-1.92 1.216-2.558z';
    default:
      return 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z';
  }
}

// 高亮搜索关键词
function highlightText(text: string, query: string): string {
  if (!query.trim()) return text;
  
  const regex = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
  return text.replace(regex, '<mark>$1</mark>');
}

onMount(() => {
  // 组件挂载时的初始化逻辑
});
  // 保留文件作为路由容器，渲染静态页面组件
</script>

<SearchPage />
