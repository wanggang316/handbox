<script lang="ts">
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { settingsState } from "$lib/states/settings.svelte";
  import { providerActions } from "$lib/states/provider.svelte";

  // 启动页停留到「主界面关键数据就绪」为止，而非一个固定时长：
  // 加载快 → 一闪而过；加载慢 → 多停一会。
  // MIN_VISIBLE：最短展示，避免画面一闪而过显得突兀。
  // MAX_WAIT：兜底超时，避免某个 IPC 卡死时永远停在启动页。
  const MIN_VISIBLE = 400;
  const MAX_WAIT = 8000;

  onMount(() => {
    if (!browser) return;

    let entered = false;
    const enter = () => {
      if (entered) return;
      entered = true;
      // replaceState：启动页不进历史栈，返回时不会再回到这里
      goto("/chat", { replaceState: true });
    };

    const delay = (ms: number) =>
      new Promise((resolve) => setTimeout(resolve, ms));

    // 与 root layout 的预加载对齐；两个方法均幂等，重复调用不会重载已有数据。
    // allSettled：任一加载失败也不阻塞进入主界面（主界面自带各自的兜底加载态）。
    const ready = Promise.allSettled([
      settingsState.loadSettings(),
      providerActions.loadProvidersWithModels(false),
    ]);

    // 就绪且至少展示 MIN_VISIBLE，或到达 MAX_WAIT 兜底，二者先到先进入。
    Promise.race([Promise.all([ready, delay(MIN_VISIBLE)]), delay(MAX_WAIT)]).then(
      enter,
    );
  });
</script>

<div class="splash">
  <div class="splash__content">
    <img
      class="splash__logo"
      src="/logo-150.png"
      alt="HandBox"
      width="80"
      height="80"
    />
    <div class="splash__brand">
      <h1 class="splash__title">HandBox</h1>
      <p class="splash__slogan">你的本地优先 AI 工作台</p>
    </div>
    <div class="splash__loader" role="status" aria-label="Loading">
      <span></span><span></span><span></span>
    </div>
  </div>
</div>

<style>
  .splash {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    /* canvas 底色，随 data-theme 自动切换深/浅 */
    background-color: var(--base-100);
    color: var(--base-content);
  }

  .splash__content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.25rem;
    animation: splash-fade-in var(--dur-slow, 300ms) var(--ease-out, ease-out)
      both;
  }

  .splash__logo {
    border-radius: 1.125rem;
    box-shadow: 0 8px 24px -12px var(--overlay);
    animation: splash-logo-float 3s var(--ease-standard, ease-in-out) infinite;
  }

  .splash__brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.375rem;
  }

  .splash__title {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--base-content);
  }

  .splash__slogan {
    margin: 0;
    font-size: 0.875rem;
    /* 设计系统弱化辅助文字色，深浅主题各有定义 */
    color: var(--ink-subtle);
  }

  .splash__loader {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .splash__loader span {
    width: 7px;
    height: 7px;
    border-radius: 9999px;
    background-color: var(--primary);
    animation: splash-dot 1.2s var(--ease-standard, ease-in-out) infinite;
  }

  .splash__loader span:nth-child(2) {
    animation-delay: 0.16s;
  }

  .splash__loader span:nth-child(3) {
    animation-delay: 0.32s;
  }

  @keyframes splash-fade-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes splash-logo-float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-5px);
    }
  }

  @keyframes splash-dot {
    0%,
    100% {
      opacity: 0.25;
      transform: scale(0.8);
    }
    50% {
      opacity: 1;
      transform: scale(1);
    }
  }

  /* 尊重系统「减少动态效果」偏好 */
  @media (prefers-reduced-motion: reduce) {
    .splash__content,
    .splash__logo,
    .splash__loader span {
      animation: none;
    }
  }
</style>
