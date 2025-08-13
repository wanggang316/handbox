<script lang="ts">
  import SettingsPanel from '$lib/components/settings/SettingsPanel.svelte';
</script>

<SettingsPanel />

<style>
.settings-container {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
}

.settings-header {
  margin-bottom: 2rem;
}

.settings-header h1 {
  margin: 0 0 0.5rem 0;
  font-size: 2rem;
  font-weight: 700;
}

.settings-header p {
  margin: 0;
  color: var(--text-secondary);
}

/* 标签页样式 */
.tabs {
  display: flex;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 2rem;
}

.tab {
  padding: 0.75rem 1.5rem;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  font-weight: 500;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.tab:hover {
  color: var(--text-primary);
}

.tab.active {
  color: var(--bg-accent);
  border-bottom-color: var(--bg-accent);
}

/* 内容区域样式 */
.section {
  margin-bottom: 2rem;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

.section h2 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
}

/* 按钮样式 */
.btn-primary {
  background: var(--bg-accent);
  color: var(--text-accent);
  border: none;
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}

.btn-primary:hover {
  opacity: 0.9;
}

.btn-secondary {
  background: var(--bg-secondary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: var(--bg-hover);
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-danger {
  background: #ef4444;
  color: white;
  border: none;
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}

.btn-danger:hover {
  opacity: 0.9;
}

/* 供应商列表样式 */
.providers-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.provider-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  background: var(--bg-secondary);
  border-radius: 8px;
  border: 1px solid var(--border-color);
}

.provider-info h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.125rem;
  font-weight: 600;
}

.provider-type {
  margin: 0 0 0.25rem 0;
  font-size: 0.875rem;
  color: var(--text-secondary);
  font-weight: 500;
}

.provider-model {
  margin: 0;
  font-size: 0.75rem;
  color: var(--text-secondary);
  opacity: 0.7;
}

.provider-actions {
  display: flex;
  gap: 0.5rem;
}

.empty-state {
  text-align: center;
  padding: 3rem 1rem;
  color: var(--text-secondary);
}

.empty-description {
  font-size: 0.875rem;
  opacity: 0.7;
  margin-top: 0.5rem;
}

/* 设置组样式 */
.setting-group {
  margin-bottom: 1.5rem;
}

.setting-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
}

.setting-label {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.setting-label input[type="checkbox"] {
  margin-right: 0.5rem;
}

.number-input,
.select-input {
  width: 200px;
  padding: 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  color: var(--text-primary);
}

/* 模态框样式 */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-primary);
  border-radius: 8px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.15);
  max-width: 500px;
  width: 90%;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  border-bottom: 1px solid var(--border-color);
}

.modal-header h3 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.close-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 0.25rem;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.close-btn:hover {
  background: var(--bg-hover);
}

.modal-content {
  padding: 1.5rem;
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
}

.form-group input,
.form-group select {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 0.875rem;
}

.form-group input:focus,
.form-group select:focus {
  outline: none;
  border-color: var(--bg-accent);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 2rem;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .settings-container {
    padding: 1rem;
  }
  
  .provider-card {
    flex-direction: column;
    align-items: flex-start;
    gap: 1rem;
  }
  
  .provider-actions {
    width: 100%;
    justify-content: flex-end;
  }
}
</style>