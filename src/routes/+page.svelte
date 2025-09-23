<script lang="ts">
import { onMount } from 'svelte';
import { browser } from '$app/environment';

onMount(() => {
  if (browser) {
    // 自动重定向到聊天页面
    window.location.href = '/chat';
  }
});
</script>

<div class="loading-container">
  <div class="loading-content">
    <div class="logo">
      <h1>HandBox</h1>
    </div>
    <div class="loading-spinner"></div>
    <p>Loading...</p>
  </div>
</div>

<style>
.loading-container {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background-color: var(--base-100);
}

.loading-content {
  text-align: center;
  color: var(--base-content);
}

.logo h1 {
  font-size: 2.5rem;
  font-weight: 700;
  margin: 0 0 2rem 0;
  color: var(--primary);
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--base-300);
  border-top-color: var(--primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin: 0 auto 1rem auto;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.loading-container p {
  margin: 0;
  color: color-mix(in oklch, var(--base-content) 80%, transparent);
  font-size: 0.875rem;
}
</style>
