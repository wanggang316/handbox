<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from '$lib/i18n';

  // Google OAuth callback: extract the auth code from the URL and postMessage it to the opener
  onMount(() => {
    const urlParams = new URLSearchParams(window.location.search);
    const code = urlParams.get('code');
    const error = urlParams.get('error');

    if (window.opener) {
      window.opener.postMessage(
        {
          type: 'google-auth-callback',
          code,
          error
        },
        window.location.origin
      );

      window.close();
    } else {
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
