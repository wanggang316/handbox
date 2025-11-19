<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Download, Trash2, Search, X, Loader2 } from 'lucide-svelte';
  import IconButton from '../ui/IconButton.svelte';
  import { artifactState } from '$lib/states/artifact.svelte';
  import type { Artifact, ArtifactType, ExecutionResult } from '$lib/types';

  let searchQuery = $state('');
  let selectedType = $state<ArtifactType | 'all'>('all');
  let selectedArtifact = $state<Artifact | null>(null);
  let executionResult = $state<ExecutionResult | null>(null);
  let isExecuting = $state(false);

  const typeIcons: Record<ArtifactType, string> = {
    shell: '🐚',
    python: '🐍',
    web: '📊'
  };

  const typeLabels: Record<ArtifactType, string> = {
    shell: 'Shell',
    python: 'Python',
    web: 'Web'
  };

  onMount(async () => {
    await loadArtifacts();
  });

  async function loadArtifacts() {
    try {
      await artifactState.loadArtifacts({
        isBuiltin: true, // 只加载内置应用
        limit: 100,
        offset: 0
      });
    } catch (error) {
      console.error('Failed to load artifacts:', error);
    }
  }

  let filteredArtifacts = $derived(() => {
    let artifacts = artifactState.artifacts;

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      artifacts = artifacts.filter(
        (a) =>
          a.name.toLowerCase().includes(query) ||
          a.description?.toLowerCase().includes(query)
      );
    }

    if (selectedType !== 'all') {
      artifacts = artifacts.filter((a) => a.type === selectedType);
    }

    return artifacts;
  });

  function selectArtifact(artifact: Artifact) {
    selectedArtifact = artifact;
    executionResult = null;
  }

  async function installArtifact(artifact: Artifact) {
    try {
      await artifactState.installArtifact({
        artifactId: artifact.id
      });
      // 重新加载列表以获取最新状态
      await loadArtifacts();
    } catch (error) {
      console.error('Failed to install artifact:', error);
    }
  }

  async function executeArtifact(artifact: Artifact) {
    if (!artifact.isInstalled) {
      await installArtifact(artifact);
    }

    isExecuting = true;
    executionResult = null;

    try {
      const result = await artifactState.executeArtifact({
        artifactId: artifact.id
      });
      executionResult = result;

      // 如果是 web 应用，在新窗口打开
      if (artifact.type === 'web' && result.success && result.stdout) {
        window.open(result.stdout, '_blank');
      }
    } catch (error) {
      console.error('Failed to execute artifact:', error);
      executionResult = {
        success: false,
        error: error instanceof Error ? error.message : 'Execution failed',
        duration: 0
      };
    } finally {
      isExecuting = false;
    }
  }

  async function deleteArtifact(artifact: Artifact) {
    if (!confirm(`确定要删除 "${artifact.name}" 吗?`)) return;

    try {
      await artifactState.deleteArtifact(artifact.id);
      if (selectedArtifact?.id === artifact.id) {
        selectedArtifact = null;
      }
    } catch (error) {
      console.error('Failed to delete artifact:', error);
    }
  }
</script>

<div class="layout">
  <!-- 侧边栏 -->
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>Artifacts</h2>
      <span class="count">{artifactState.artifacts.length}</span>
    </div>

    <!-- 搜索 -->
    <div class="search-box">
      <Search size={16} class="search-icon" />
      <input
        type="text"
        placeholder="搜索应用..."
        bind:value={searchQuery}
        class="search-input"
      />
      {#if searchQuery}
        <IconButton
          icon={X}
          ariaLabel="清除搜索"
          onclick={() => (searchQuery = '')}
          customClass="clear-btn"
        />
      {/if}
    </div>

    <!-- 类型过滤 -->
    <div class="type-filters">
      <button
        class="type-btn"
        class:active={selectedType === 'all'}
        onclick={() => (selectedType = 'all')}
      >
        All
      </button>
      {#each ['shell', 'python', 'web'] as type}
        <button
          class="type-btn"
          class:active={selectedType === type}
          onclick={() => (selectedType = type as ArtifactType)}
        >
          {typeIcons[type as ArtifactType]} {typeLabels[type as ArtifactType]}
        </button>
      {/each}
    </div>

    <!-- 应用列表 -->
    <div class="artifact-list">
      {#if artifactState.isLoading}
        <div class="loading">
          <Loader2 size={24} class="spin" />
          <p>加载中...</p>
        </div>
      {:else if filteredArtifacts().length === 0}
        <div class="empty">
          <p>没有找到应用</p>
        </div>
      {:else}
        {#each filteredArtifacts() as artifact (artifact.id)}
          <div
            class="artifact-item"
            class:selected={selectedArtifact?.id === artifact.id}
            role="button"
            tabindex="0"
            onclick={() => selectArtifact(artifact)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                selectArtifact(artifact);
              }
            }}
          >
            <div class="artifact-icon">
              {artifact.icon || typeIcons[artifact.type]}
            </div>
            <div class="artifact-info">
              <h4>{artifact.name}</h4>
              <p class="artifact-meta">
                {typeLabels[artifact.type]}
                {#if artifact.isInstalled}
                  <span class="badge installed">已安装</span>
                {/if}
              </p>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </aside>

  <!-- 主内容区 -->
  <main class="main">
    {#if selectedArtifact}
      <div class="detail-view">
        <!-- 头部 -->
        <div class="detail-header">
          <div class="title-section">
            <div class="artifact-icon-large">
              {selectedArtifact.icon || typeIcons[selectedArtifact.type]}
            </div>
            <div>
              <h1>{selectedArtifact.name}</h1>
              <div class="meta-info">
                <span class="badge type">{typeLabels[selectedArtifact.type]}</span>
                {#if selectedArtifact.isInstalled}
                  <span class="badge installed">已安装</span>
                {/if}
                {#if selectedArtifact.author}
                  <span class="author">by {selectedArtifact.author}</span>
                {/if}
              </div>
            </div>
          </div>

          <div class="actions">
            {#if !selectedArtifact.isInstalled}
              <button
                class="btn btn-primary"
                onclick={() => installArtifact(selectedArtifact!)}
                disabled={artifactState.isLoading}
              >
                <Download size={16} />
                安装
              </button>
            {/if}
            <button
              class="btn btn-primary"
              onclick={() => executeArtifact(selectedArtifact!)}
              disabled={isExecuting || artifactState.isLoading}
            >
              {#if isExecuting}
                <Loader2 size={16} class="spin" />
              {:else}
                <Play size={16} />
              {/if}
              运行
            </button>
            {#if !selectedArtifact.isBuiltin}
              <button
                class="btn btn-danger"
                onclick={() => deleteArtifact(selectedArtifact!)}
                aria-label="删除"
              >
                <Trash2 size={16} />
              </button>
            {/if}
          </div>
        </div>

        <!-- 描述 -->
        {#if selectedArtifact.description}
          <div class="description">
            <p>{selectedArtifact.description}</p>
          </div>
        {/if}

        <!-- 标签 -->
        {#if selectedArtifact.tags.length > 0}
          <div class="tags">
            {#each selectedArtifact.tags as tag}
              <span class="tag">{tag}</span>
            {/each}
          </div>
        {/if}

        <!-- 技术信息 -->
        <div class="tech-info">
          <div class="info-row">
            <span class="label">入口文件:</span>
            <code>{selectedArtifact.entryFile}</code>
          </div>
          {#if selectedArtifact.installedVersion}
            <div class="info-row">
              <span class="label">版本:</span>
              <code>{selectedArtifact.installedVersion}</code>
            </div>
          {/if}
          {#if selectedArtifact.runCount > 0}
            <div class="info-row">
              <span class="label">运行次数:</span>
              <span>{selectedArtifact.runCount}</span>
            </div>
          {/if}
        </div>

        <!-- 执行结果 -->
        {#if executionResult}
          <div class="execution-result" class:success={executionResult.success} class:error={!executionResult.success}>
            <div class="result-header">
              <h3>{executionResult.success ? '✅ 执行成功' : '❌ 执行失败'}</h3>
              <span class="duration">{executionResult.duration}ms</span>
            </div>

            {#if executionResult.stdout}
              <div class="output">
                <h4>标准输出:</h4>
                <pre>{executionResult.stdout}</pre>
              </div>
            {/if}

            {#if executionResult.stderr}
              <div class="output stderr">
                <h4>标准错误:</h4>
                <pre>{executionResult.stderr}</pre>
              </div>
            {/if}

            {#if executionResult.error}
              <div class="output error-msg">
                <h4>错误信息:</h4>
                <pre>{executionResult.error}</pre>
              </div>
            {/if}

            {#if executionResult.exitCode !== undefined}
              <div class="exit-code">
                退出码: <code>{executionResult.exitCode}</code>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {:else}
      <div class="empty-state">
        <div class="empty-icon">📦</div>
        <h3>选择一个应用</h3>
        <p>从左侧列表中选择一个应用以查看详情</p>
      </div>
    {/if}
  </main>
</div>

<style>
  .layout {
    display: flex;
    height: 100vh;
    background: var(--base-100);
  }

  /* === 侧边栏 === */
  .sidebar {
    width: 320px;
    background: var(--base-200);
    border-right: 1px solid var(--base-300);
    display: flex;
    flex-direction: column;
  }

  .sidebar-header {
    padding: 1.5rem;
    border-bottom: 1px solid var(--base-300);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sidebar-header h2 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
  }

  .count {
    background: var(--base-300);
    color: var(--base-content);
    padding: 0.25rem 0.625rem;
    border-radius: 12px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  /* 搜索框 */
  .search-box {
    position: relative;
    padding: 1rem;
    border-bottom: 1px solid var(--base-300);
  }

  .search-icon {
    position: absolute;
    left: 1.75rem;
    top: 50%;
    transform: translateY(-50%);
    color: color-mix(in oklch, var(--base-content) 60%, transparent);
  }

  .search-input {
    width: 100%;
    padding: 0.625rem 0.75rem 0.625rem 2.5rem;
    border: 1px solid var(--base-300);
    border-radius: 8px;
    background: var(--base-100);
    color: var(--base-content);
    font-size: 0.875rem;
  }

  .search-input:focus {
    outline: 2px solid var(--primary);
    outline-offset: 0;
  }

  :global(.clear-btn) {
    position: absolute;
    right: 1.25rem;
    top: 50%;
    transform: translateY(-50%);
  }

  /* 类型过滤 */
  .type-filters {
    display: flex;
    gap: 0.5rem;
    padding: 1rem;
    border-bottom: 1px solid var(--base-300);
    flex-wrap: wrap;
  }

  .type-btn {
    padding: 0.5rem 0.875rem;
    border: 1px solid var(--base-300);
    border-radius: 6px;
    background: var(--base-100);
    color: var(--base-content);
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .type-btn:hover {
    background: var(--base-200);
  }

  .type-btn.active {
    background: var(--primary);
    color: var(--primary-content);
    border-color: var(--primary);
  }

  /* 应用列表 */
  .artifact-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .artifact-item {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.875rem;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.2s;
    margin-bottom: 0.5rem;
  }

  .artifact-item:hover {
    background: var(--base-300);
  }

  .artifact-item.selected {
    background: var(--primary);
    color: var(--primary-content);
  }

  .artifact-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--base-100);
    border-radius: 8px;
    font-size: 1.5rem;
  }

  .artifact-item.selected .artifact-icon {
    background: color-mix(in oklch, var(--primary-content) 20%, transparent);
  }

  .artifact-info {
    flex: 1;
    min-width: 0;
  }

  .artifact-info h4 {
    margin: 0 0 0.25rem 0;
    font-size: 0.9375rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artifact-meta {
    margin: 0;
    font-size: 0.8125rem;
    color: color-mix(in oklch, var(--base-content) 70%, transparent);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .artifact-item.selected .artifact-meta {
    color: color-mix(in oklch, var(--primary-content) 80%, transparent);
  }

  /* === 主内容区 === */
  .main {
    flex: 1;
    overflow-y: auto;
    padding: 2rem;
  }

  .detail-view {
    max-width: 800px;
    margin: 0 auto;
  }

  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 2rem;
    gap: 2rem;
  }

  .title-section {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
  }

  .artifact-icon-large {
    width: 64px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--base-200);
    border-radius: 12px;
    font-size: 2rem;
  }

  .detail-header h1 {
    margin: 0 0 0.5rem 0;
    font-size: 1.75rem;
    font-weight: 700;
  }

  .meta-info {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
  }

  .badge {
    padding: 0.25rem 0.625rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .badge.type {
    background: var(--base-200);
    color: var(--base-content);
  }

  .badge.installed {
    background: color-mix(in oklch, var(--success) 20%, transparent);
    color: var(--success);
  }

  .author {
    color: color-mix(in oklch, var(--base-content) 60%, transparent);
    font-size: 0.875rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.625rem 1rem;
    border: none;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--primary);
    color: var(--primary-content);
  }

  .btn-primary:hover:not(:disabled) {
    background: color-mix(in oklch, var(--primary) 90%, black);
  }

  .btn-danger {
    background: var(--error);
    color: var(--error-content);
  }

  .btn-danger:hover:not(:disabled) {
    background: color-mix(in oklch, var(--error) 90%, black);
  }

  .description {
    margin-bottom: 1.5rem;
    padding: 1.25rem;
    background: var(--base-200);
    border-radius: 8px;
    line-height: 1.6;
  }

  .tags {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 1.5rem;
  }

  .tag {
    padding: 0.375rem 0.75rem;
    background: var(--base-200);
    border-radius: 6px;
    font-size: 0.8125rem;
    color: var(--base-content);
  }

  .tech-info {
    margin-bottom: 1.5rem;
    padding: 1.25rem;
    background: var(--base-200);
    border-radius: 8px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .info-row:last-child {
    margin-bottom: 0;
  }

  .info-row .label {
    font-weight: 600;
    min-width: 80px;
    font-size: 0.875rem;
  }

  .info-row code {
    background: var(--base-300);
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.8125rem;
  }

  /* 执行结果 */
  .execution-result {
    margin-top: 1.5rem;
    padding: 1.25rem;
    border-radius: 8px;
    border: 2px solid;
  }

  .execution-result.success {
    background: color-mix(in oklch, var(--success) 10%, transparent);
    border-color: var(--success);
  }

  .execution-result.error {
    background: color-mix(in oklch, var(--error) 10%, transparent);
    border-color: var(--error);
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .result-header h3 {
    margin: 0;
    font-size: 1.125rem;
  }

  .duration {
    font-size: 0.875rem;
    color: color-mix(in oklch, var(--base-content) 60%, transparent);
  }

  .output {
    margin-top: 1rem;
  }

  .output h4 {
    margin: 0 0 0.5rem 0;
    font-size: 0.875rem;
    font-weight: 600;
  }

  .output pre {
    background: var(--base-100);
    padding: 1rem;
    border-radius: 6px;
    overflow-x: auto;
    margin: 0;
    font-size: 0.8125rem;
    line-height: 1.5;
  }

  .exit-code {
    margin-top: 1rem;
    font-size: 0.875rem;
  }

  /* 空状态 */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: color-mix(in oklch, var(--base-content) 60%, transparent);
  }

  .empty-icon {
    font-size: 4rem;
    margin-bottom: 1rem;
  }

  .empty-state h3 {
    margin: 0 0 0.5rem 0;
    font-size: 1.25rem;
    color: var(--base-content);
  }

  .empty, .loading {
    text-align: center;
    padding: 2rem;
    color: color-mix(in oklch, var(--base-content) 60%, transparent);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
</style>
