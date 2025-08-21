<script lang="ts">
  import ArtifactsLayout from '$lib/components/artifacts/ArtifactsLayout.svelte';
</script>

<ArtifactsLayout />

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