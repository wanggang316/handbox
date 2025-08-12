<script lang="ts">
import { onMount } from 'svelte';
import { 
  searchResults,
  searchSuggestions,
  searchLoading,
  searchActions,
  chatActions
} from '$lib/stores';

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
    await searchActions.search({ query: searchQuery, types });
  } catch (error) {
    console.error('Search failed:', error);
  }
}

// 处理搜索输入
function handleSearchInput() {
  if (searchQuery.trim()) {
    performSearch();
  } else {
    searchActions.clearResults();
  }
}

// 处理键盘事件
function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    performSearch();
  }
}

// 跳转到会话
function goToSession(sessionId: string) {
  chatActions.switchToSession(sessionId);
  window.location.href = '/chat';
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
</script>

<div class="search-container">
  <div class="search-header">
    <h1>Search</h1>
    <p>Search through your conversations, artifacts, and more</p>
  </div>

  <!-- 搜索输入区域 -->
  <div class="search-input-section">
    <div class="search-input-container">
      <svg class="search-icon" width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
      <input
        type="text"
        placeholder="Search conversations, artifacts, and more..."
        bind:value={searchQuery}
        oninput={handleSearchInput}
        onkeydown={handleKeydown}
        class="search-input"
      />
      {#if searchQuery}
        <button 
          class="clear-btn"
          onclick={() => {
            searchQuery = '';
            searchActions.clearResults();
          }}
        >
          <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      {/if}
    </div>

    <!-- 搜索过滤器 -->
    <div class="search-filters">
      {#each filterOptions as option (option.value)}
        <button 
          class="filter-btn"
          class:active={selectedFilter === option.value}
          onclick={() => {
            selectedFilter = option.value;
            if (searchQuery.trim()) performSearch();
          }}
        >
          {option.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- 搜索结果 -->
  <div class="search-results">
    {#if $searchLoading}
      <div class="loading-state">
        <div class="loading-spinner"></div>
        <p>Searching...</p>
      </div>
    {:else if searchQuery && $searchResults.length === 0 && !$searchLoading}
      <div class="empty-results">
        <div class="empty-icon">
          <svg width="48" height="48" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </div>
        <h3>No results found</h3>
        <p>Try adjusting your search terms or filters</p>
      </div>
    {:else if $searchResults.length > 0}
      <div class="results-header">
        <h3>{$searchResults.length} result{$searchResults.length !== 1 ? 's' : ''} found</h3>
      </div>
      
      <div class="results-list">
        {#each $searchResults as result (result.id)}
          <div 
            class="result-item"
            onclick={() => {
              if (result.type === 'session' || result.type === 'message') {
                goToSession(result.sessionId || result.id);
              } else if (result.type === 'artifact') {
                window.location.href = '/artifacts';
              }
            }}
          >
            <div class="result-icon">
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={getResultIcon(result.type)} />
              </svg>
            </div>
            
            <div class="result-content">
              <div class="result-header">
                <h4>{@html highlightText(result.title, searchQuery)}</h4>
                <span class="result-type">{result.type}</span>
              </div>
              
              <p class="result-snippet">
                {@html highlightText(result.content, searchQuery)}
              </p>
              
              <div class="result-meta">
                <span class="result-date">{new Date(result.timestamp).toLocaleDateString()}</span>
                {#if result.sessionId}
                  <span class="result-session">in session {result.sessionId}</span>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="search-suggestions">
        <h3>Search Tips</h3>
        <div class="tips-grid">
          <div class="tip">
            <div class="tip-icon">
              <svg width="24" height="24" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
              </svg>
            </div>
            <h4>Search Messages</h4>
            <p>Find specific messages in your conversation history</p>
          </div>
          
          <div class="tip">
            <div class="tip-icon">
              <svg width="24" height="24" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
              </svg>
            </div>
            <h4>Find Sessions</h4>
            <p>Locate conversation sessions by title or content</p>
          </div>
          
          <div class="tip">
            <div class="tip-icon">
              <svg width="24" height="24" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547A1.934 1.934 0 004 17.693v3.621l2.053-.410a6 6 0 013.86-.517l.318.158a6 6 0 003.86.517L16.947 21v-3.621c0-.987.428-1.92 1.216-2.558z" />
              </svg>
            </div>
            <h4>Search Artifacts</h4>
            <p>Find your created artifacts by name or content</p>
          </div>
          
          <div class="tip">
            <div class="tip-icon">
              <svg width="24" height="24" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h4>Use Filters</h4>
            <p>Narrow down your search with the filter buttons above</p>
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

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