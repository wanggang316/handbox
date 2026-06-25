<!--
  Quick Action 浮层占位页。

  仅渲染一张铺满窗口的圆角主题卡片，用于在 frameless / transparent 的 NSPanel 中
  目视确认无原生标题栏与透明背景（VAL-OVERLAY-021）。真正的 composer 由后续 feature
  (qa-route-shell 等) 实现——此处不接 input / Esc / i18n / agent。
-->
<div
  class="flex h-full w-full items-center justify-center rounded-xl bg-[var(--bg-card)] text-[var(--base-content)] shadow-2xl"
>
  <span class="text-sm opacity-60">Quick Action</span>
</div>

<style>
  /* 透明窗口：让 body 背景透出，仅卡片可见，便于验证 frameless 圆角浮层。 */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
