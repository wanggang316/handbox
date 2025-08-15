<script lang="ts">
  import ChatView from '$lib/components/chat/ChatView.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';

  let sessionId = $state('');

  // 从 URL 参数获取会话 ID
  onMount(() => {
    const urlParams = $page.url.searchParams;
    sessionId = urlParams.get('id') || '';
    console.log('Current session ID:', sessionId);
  });

  // 监听 URL 变化
  $effect(() => {
    const urlParams = $page.url.searchParams;
    sessionId = urlParams.get('id') || '';
    console.log('Session ID changed to:', sessionId);
  });
</script>

<!-- 聊天视图（嵌入到主布局右侧时，也可单独使用） -->
<div class="flex-1 flex flex-col">
  <ChatView {sessionId} />
</div>