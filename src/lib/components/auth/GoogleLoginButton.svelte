<script lang="ts">
  import { googleLogin } from '$lib/api/auth';
  import { userStore } from '$lib/stores';
  import { AppError } from '$lib/api';

  interface Props {
    onSuccess?: () => void;
    onError?: (error: AppError) => void;
  }

  let { onSuccess, onError }: Props = $props();

  // Google OAuth 配置
  const GOOGLE_CLIENT_ID = import.meta.env.VITE_GOOGLE_CLIENT_ID || '';
  const REDIRECT_URI = import.meta.env.VITE_GOOGLE_REDIRECT_URI || 'http://localhost:5173/auth/callback';

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);

  /**
   * 处理 Google 登录
   */
  async function handleGoogleLogin() {
    errorMessage = null;
    isLoading = true;

    try {
      // 构建 Google OAuth URL
      const authUrl = new URL('https://accounts.google.com/o/oauth2/v2/auth');
      authUrl.searchParams.set('client_id', GOOGLE_CLIENT_ID);
      authUrl.searchParams.set('redirect_uri', REDIRECT_URI);
      authUrl.searchParams.set('response_type', 'code');
      authUrl.searchParams.set('scope', 'email profile openid');
      authUrl.searchParams.set('access_type', 'offline');
      authUrl.searchParams.set('prompt', 'consent');

      // 打开 Google 登录页面
      const authWindow = window.open(
        authUrl.toString(),
        'Google Login',
        'width=500,height=600,left=100,top=100'
      );

      // 监听回调消息
      window.addEventListener('message', handleAuthCallback);
    } catch (error) {
      console.error('Google 登录失败:', error);
      errorMessage = '启动 Google 登录失败';
      isLoading = false;

      if (onError && error instanceof AppError) {
        onError(error);
      }
    }
  }

  /**
   * 处理 OAuth 回调
   */
  async function handleAuthCallback(event: MessageEvent) {
    // 验证来源
    if (event.origin !== window.location.origin) {
      return;
    }

    const { type, code, error } = event.data;

    if (type !== 'google-auth-callback') {
      return;
    }

    // 移除监听器
    window.removeEventListener('message', handleAuthCallback);

    if (error) {
      errorMessage = '授权失败: ' + error;
      isLoading = false;
      return;
    }

    if (!code) {
      errorMessage = '未获取到授权码';
      isLoading = false;
      return;
    }

    try {
      // 调用后端登录接口
      const response = await googleLogin({
        code,
        redirectUri: REDIRECT_URI
      });

      // 更新用户状态
      userStore.setUser(response.user, response.accessToken);

      // 保存刷新令牌
      if (typeof window !== 'undefined') {
        localStorage.setItem('refreshToken', response.refreshToken);
      }

      isLoading = false;
      onSuccess?.();
    } catch (error) {
      console.error('登录失败:', error);

      if (error instanceof AppError) {
        errorMessage = error.message;
        onError?.(error);
      } else {
        errorMessage = '登录失败，请重试';
      }

      isLoading = false;
    }
  }
</script>

<button
  type="button"
  onclick={handleGoogleLogin}
  disabled={isLoading}
  class="
    flex items-center justify-center gap-3
    w-full px-4 py-3
    bg-white hover:bg-gray-50
    border border-base-300 rounded-lg
    text-base-content
    transition-colors duration-200
    disabled:opacity-50 disabled:cursor-not-allowed
    shadow-sm hover:shadow-md
  "
>
  {#if isLoading}
    <div class="w-5 h-5 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
    <span>登录中...</span>
  {:else}
    <!-- Google Logo SVG -->
    <svg class="w-5 h-5" viewBox="0 0 24 24">
      <path
        fill="#4285F4"
        d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
      />
      <path
        fill="#34A853"
        d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
      />
      <path
        fill="#FBBC05"
        d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
      />
      <path
        fill="#EA4335"
        d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
      />
    </svg>
    <span>使用 Google 账号登录</span>
  {/if}
</button>

{#if errorMessage}
  <div class="mt-2 text-sm text-error">
    {errorMessage}
  </div>
{/if}
