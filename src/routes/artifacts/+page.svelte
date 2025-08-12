<script lang="ts">
import { onMount } from 'svelte';
import { 
  artifacts as artifactsStore,
  filteredArtifacts as filteredArtifactsStore,
  artifactActions
} from '$lib/stores';
import type { Artifact } from '$lib/types';

let selectedArtifact: Artifact | null = $state(null);
let showCreateModal = $state(false);
let searchQuery = $state('');

// 过滤后的 artifacts 列表（基于全局 store 再次做本地文本过滤）
let filteredArtifacts = $derived($filteredArtifactsStore.filter(artifact => 
  artifact.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
  (artifact.description || '').toLowerCase().includes(searchQuery.toLowerCase())
));

// 选择 artifact
function selectArtifact(artifact: Artifact) {
  selectedArtifact = artifact;
}

// 删除 artifact
async function deleteArtifact(artifactId: string) {
  if (confirm('Are you sure you want to delete this artifact?')) {
    try {
      await artifactActions.deleteArtifact(artifactId);
      if (selectedArtifact?.id === artifactId) {
        selectedArtifact = null;
      }
    } catch (error) {
      console.error('Failed to delete artifact:', error);
    }
  }
}

// 获取 artifact 类型的图标（统一使用 code 样式）
function getArtifactIcon(): string {
  return 'M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4';
}

// 获取颜色类（统一使用 code 样式）
function getArtifactColorClass(): string {
  return 'artifact-code';
}

onMount(() => {
  artifactActions.loadArtifacts();
});
</script>

<div class="artifacts-layout">
  <!-- 侧边栏 -->
  <aside class="artifacts-sidebar">
    <div class="sidebar-header">
      <h2>Artifacts</h2>
      <button 
        class="create-btn"
        onclick={() => showCreateModal = true}
      >
        <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        Create
      </button>
    </div>
    
    <!-- 搜索框 -->
    <div class="search-container">
      <input
        type="text"
        placeholder="Search artifacts..."
        bind:value={searchQuery}
        class="search-input"
      />
      <svg class="search-icon" width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    </div>
    
    <!-- Artifacts 列表 -->
    <div class="artifacts-list">
      {#each filteredArtifacts as artifact (artifact.id)}
        <div 
          class="artifact-item"
          class:selected={selectedArtifact?.id === artifact.id}
          onclick={() => selectArtifact(artifact)}
        >
          <div class="artifact-icon {getArtifactColorClass()}">
            <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={getArtifactIcon()} />
            </svg>
          </div>
          <div class="artifact-info">
            <h4>{artifact.name}</h4>
            <p class="artifact-type">artifact</p>
            <p class="artifact-date">{new Date(artifact.createdAt).toLocaleDateString()}</p>
          </div>
          <button 
            class="delete-btn"
            onclick={(e) => {
              e.stopPropagation();
              deleteArtifact(artifact.id);
            }}
          >
            <svg width="14" height="14" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        </div>
      {/each}
      
      {#if filteredArtifacts.length === 0}
        <div class="empty-state">
          {#if searchQuery}
            <p>No artifacts found</p>
            <p class="empty-description">Try adjusting your search query</p>
          {:else}
            <p>No artifacts yet</p>
            <p class="empty-description">Create your first artifact to get started</p>
          {/if}
        </div>
      {/if}
    </div>
  </aside>

  <!-- 主内容区 -->
  <main class="artifact-main">
    {#if selectedArtifact}
      <div class="artifact-viewer">
        <div class="artifact-header">
          <div class="artifact-title-section">
            <div class="artifact-icon {getArtifactColorClass()}">
              <svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={getArtifactIcon()} />
              </svg>
            </div>
            <div>
              <h1>{selectedArtifact.name}</h1>
              <div class="artifact-meta">
                <span class="artifact-type-badge">artifact</span>
                <span class="artifact-date">Created {new Date(selectedArtifact.createdAt).toLocaleDateString()}</span>
              </div>
            </div>
          </div>
          
          <div class="artifact-actions">
            <button class="action-btn">
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
              Edit
            </button>
            <button class="action-btn">
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
              Copy
            </button>
            <button class="action-btn">
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Download
            </button>
          </div>
        </div>
        
        <div class="artifact-content">
          <div class="content-container">
            <div class="text-content">{@html selectedArtifact.description || 'No content'}</div>
          </div>
        </div>
      </div>
    {:else}
      <div class="no-selection">
        <div class="no-selection-icon">
          <svg width="64" height="64" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547A1.934 1.934 0 004 17.693v3.621l2.053-.410a6 6 0 013.86-.517l.318.158a6 6 0 003.86.517L16.947 21v-3.621c0-.987.428-1.92 1.216-2.558z" />
          </svg>
        </div>
        <h3>No artifact selected</h3>
        <p>Select an artifact from the sidebar to view its content</p>
      </div>
    {/if}
  </main>
</div>

<style>
.artifacts-layout {
  display: flex;
  height: 100vh;
}

/* 侧边栏样式 */
.artifacts-sidebar {
  width: 320px;
  background-color: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 1rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sidebar-header h2 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.create-btn {
  background: var(--bg-accent);
  color: var(--text-accent);
  border: none;
  border-radius: 6px;
  padding: 0.5rem 0.75rem;
  cursor: pointer;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  transition: opacity 0.2s;
}

.create-btn:hover {
  opacity: 0.9;
}

.search-container {
  padding: 1rem;
  position: relative;
}

.search-input {
  width: 100%;
  padding: 0.5rem 0.75rem 0.5rem 2.5rem;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 0.875rem;
}

.search-input:focus {
  outline: none;
  border-color: var(--bg-accent);
}

.search-icon {
  position: absolute;
  left: 1.75rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-secondary);
}

.artifacts-list {
  flex: 1;
  overflow-y: auto;
  padding: 0.5rem;
}

.artifact-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s;
  position: relative;
}

.artifact-item:hover {
  background-color: var(--bg-hover);
}

.artifact-item.selected {
  background-color: var(--bg-accent);
  color: var(--text-accent);
}

.artifact-icon {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.artifact-code {
  background-color: #3b82f6;
  color: white;
}

.artifact-document {
  background-color: #10b981;
  color: white;
}

.artifact-image {
  background-color: #f59e0b;
  color: white;
}

.artifact-data {
  background-color: #8b5cf6;
  color: white;
}

.artifact-default {
  background-color: var(--bg-secondary);
  color: var(--text-secondary);
}

.artifact-info {
  flex: 1;
  min-width: 0;
}

.artifact-info h4 {
  margin: 0 0 0.25rem 0;
  font-size: 0.875rem;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.artifact-type {
  margin: 0 0 0.25rem 0;
  font-size: 0.75rem;
  text-transform: capitalize;
  opacity: 0.8;
}

.artifact-date {
  margin: 0;
  font-size: 0.75rem;
  opacity: 0.6;
}

.delete-btn {
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 4px;
  opacity: 0;
  transition: all 0.2s;
}

.artifact-item:hover .delete-btn {
  opacity: 1;
}

.delete-btn:hover {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.empty-state {
  text-align: center;
  padding: 2rem 1rem;
  color: var(--text-secondary);
}

.empty-description {
  font-size: 0.875rem;
  opacity: 0.7;
  margin-top: 0.5rem;
}

/* 主内容区样式 */
.artifact-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.artifact-viewer {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.artifact-header {
  padding: 1.5rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.artifact-title-section {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.artifact-header h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
}

.artifact-meta {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-top: 0.5rem;
}

.artifact-type-badge {
  background: var(--bg-secondary);
  color: var(--text-secondary);
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 500;
  text-transform: capitalize;
}

.artifact-date {
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.artifact-actions {
  display: flex;
  gap: 0.5rem;
}

.action-btn {
  padding: 0.5rem 0.75rem;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-primary);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  transition: all 0.2s;
}

.action-btn:hover {
  background: var(--bg-hover);
}

.artifact-content {
  flex: 1;
  overflow: auto;
  padding: 1.5rem;
}

.content-container {
  max-width: 100%;
}

.artifact-content pre {
  background: var(--bg-secondary);
  padding: 1rem;
  border-radius: 6px;
  overflow-x: auto;
  margin: 0;
}

.artifact-content code {
  font-family: 'Fira Code', 'Monaco', 'Consolas', monospace;
  font-size: 0.875rem;
  line-height: 1.5;
}

.artifact-content img {
  max-width: 100%;
  height: auto;
  border-radius: 6px;
}

.text-content {
  line-height: 1.6;
}

.no-selection {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 2rem;
  color: var(--text-secondary);
}

.no-selection-icon {
  margin-bottom: 1rem;
  opacity: 0.5;
}

.no-selection h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.no-selection p {
  margin: 0;
  opacity: 0.7;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .artifacts-sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    z-index: 1000;
    transform: translateX(-100%);
    transition: transform 0.3s ease;
  }
  
  .artifact-main {
    margin-left: 0;
  }
  
  .artifact-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 1rem;
  }
  
  .artifact-actions {
    width: 100%;
    justify-content: flex-end;
  }
}
</style>