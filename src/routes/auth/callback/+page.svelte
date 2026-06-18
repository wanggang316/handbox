<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from '$lib/i18n';

  /**
   * Google OAuth 回调页面
   *
   * 从 URL 中提取授权码，并通过 postMessage 发送给父窗口
   */
  onMount(() => {
    const urlParams = new URLSearchParams(window.location.search);
    const code = urlParams.get('code');
    const error = urlParams.get('error');

    // 向父窗口发送消息
    if (window.opener) {
      window.opener.postMessage(
        {
          type: 'google-auth-callback',
          code,
          error
        },
        window.location.origin
      );

      // 关闭当前窗口
      window.close();
    } else {
      // 如果不是弹出窗口，显示错误信息
      console.error('此页面应该在弹出窗口中打开');
    }
  });
</script>

<div class="flex items-center justify-center min-h-screen bg-base-100">
  <div class="text-center">
    <div class="w-16 h-16 mx-auto mb-4 border-4 border-primary border-t-transparent rounded-full animate-spin"></div>
    <p class="text-base-content/80">{t('ui.processingLogin')}</p>
  </div>
</div>
