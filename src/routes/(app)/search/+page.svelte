<script lang="ts">
  import SearchPage from '$lib/components/search/SearchPage.svelte';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { searchState } from '$lib/stores';
  import { chatState } from '$lib/stores';

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
  chatState.switchToChat(chatId);
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

<style>
.search-container {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
}

.search-header {
  margin-bottom: 2rem;
}

.search-header h1 {
  margin: 0 0 0.5rem 0;
  font-size: 2rem;
  font-weight: 700;
}

.search-header p {
  margin: 0;
  color: var(--text-secondary);
}

/* 搜索输入区域 */
.search-input-section {
  margin-bottom: 2rem;
}

.search-input-container {
  position: relative;
  margin-bottom: 1rem;
}

.search-icon {
  position: absolute;
  left: 1rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-secondary);
}

.search-input {
  width: 100%;
  padding: 0.75rem 1rem 0.75rem 3rem;
  border: 2px solid var(--border-color);
  border-radius: 12px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 1rem;
  transition: border-color 0.2s;
}

.search-input:focus {
  outline: none;
  border-color: var(--bg-accent);
}

.clear-btn {
  position: absolute;
  right: 0.75rem;
  top: 50%;
  transform: translateY(-50%);
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 4px;
  transition: all 0.2s;
}

.clear-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.search-filters {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.filter-btn {
  padding: 0.5rem 1rem;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 20px;
  cursor: pointer;
  color: var(--text-secondary);
  font-weight: 500;
  transition: all 0.2s;
}

.filter-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.filter-btn.active {
  background: var(--bg-accent);
  color: var(--text-accent);
  border-color: var(--bg-accent);
}

/* 搜索结果 */
.search-results {
  min-height: 300px;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 3rem;
  color: var(--text-secondary);
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color);
  border-top-color: var(--bg-accent);
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 1rem;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.empty-results {
  text-align: center;
  padding: 3rem;
  color: var(--text-secondary);
}

.empty-icon {
  margin-bottom: 1rem;
  opacity: 0.5;
}

.empty-results h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.empty-results p {
  margin: 0;
  opacity: 0.7;
}

.results-header {
  margin-bottom: 1.5rem;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid var(--border-color);
}

.results-header h3 {
  margin: 0;
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--text-secondary);
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.result-item {
  display: flex;
  gap: 1rem;
  padding: 1.5rem;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.result-item:hover {
  background: var(--bg-hover);
  border-color: var(--bg-accent);
}

.result-icon {
  width: 32px;
  height: 32px;
  background: var(--bg-accent);
  color: var(--text-accent);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.result-content {
  flex: 1;
  min-width: 0;
}

.result-header {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 0.5rem;
}

.result-header h4 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  flex: 1;
}

.result-type {
  background: var(--bg-primary);
  color: var(--text-secondary);
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 500;
  text-transform: capitalize;
}

.result-snippet {
  margin: 0 0 0.75rem 0;
  color: var(--text-secondary);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.result-meta {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.result-session {
  opacity: 0.7;
}

/* 搜索建议 */
.search-suggestions {
  padding: 2rem;
}

.search-suggestions h3 {
  margin: 0 0 1.5rem 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.tips-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1.5rem;
}

.tip {
  padding: 1.5rem;
  background: var(--bg-secondary);
  border-radius: 8px;
  text-align: center;
}

.tip-icon {
  margin-bottom: 1rem;
  color: var(--bg-accent);
}

.tip h4 {
  margin: 0 0 0.5rem 0;
  font-size: 1rem;
  font-weight: 600;
}

.tip p {
  margin: 0;
  color: var(--text-secondary);
  font-size: 0.875rem;
  line-height: 1.5;
}

/* 高亮样式 */
:global(mark) {
  background: #fef3c7;
  color: #92400e;
  padding: 0.125rem 0.25rem;
  border-radius: 2px;
  font-weight: 500;
}

:global([data-theme="dark"] mark) {
  background: #fbbf24;
  color: #1f2937;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .search-container {
    padding: 1rem;
  }
  
  .search-filters {
    justify-content: center;
  }
  
  .tips-grid {
    grid-template-columns: 1fr;
  }
  
  .result-item {
    padding: 1rem;
  }
  
  .result-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
  }
}
</style>