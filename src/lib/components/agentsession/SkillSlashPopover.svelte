<script lang="ts">
  import { t } from "$lib/i18n";
  import type { SkillInfo } from "$lib/types";

  interface Props {
    /** 已过滤的候选 skill（仅未禁用、已按 query 匹配）。 */
    items: SkillInfo[];
    /** 当前高亮项索引；items 非空时为 [0, items.length)，空时为 -1。 */
    highlightedIndex: number;
    /** 点击某项时回调（同 Enter 选中）。 */
    onSelect: (skill: SkillInfo) => void;
    /** hover 某项时同步高亮索引，使键盘/鼠标高亮统一。 */
    onHover: (index: number) => void;
  }

  let { items, highlightedIndex, onSelect, onHover }: Props = $props();

  // 列表容器引用：高亮变化时把当前项滚入可视区，使键盘（↑/↓、Ctrl|Cmd+N/P）
  // 移动高亮越过 max-h 边界时列表跟随滚动。`nearest` 只在必要时滚动，已可见则不动。
  let listRef = $state<HTMLDivElement>();
  $effect(() => {
    const idx = highlightedIndex;
    if (idx < 0 || !listRef) return;
    const active = listRef.querySelector('[aria-selected="true"]');
    if (active instanceof HTMLElement) {
      active.scrollIntoView({ block: "nearest" });
    }
  });
</script>

<!--
  锚定 textarea 的 skill 自动补全浮层。父组件用 absolute 定位容器把本组件钉在
  输入框上方（bottom-full），故此处只负责列表内容与高亮渲染。键盘行为
  （↑/↓/Enter/Escape）由父组件在 textarea 的 keydown 上统一路由。
  a11y：listbox/option role + aria-selected，高亮态可程序读取（VAL-SLASH-026）。
-->
<div
  bind:this={listRef}
  class="max-h-60 w-72 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-200 py-1 shadow-lg"
  role="listbox"
  aria-label={t("agent.slash.ariaLabel")}
>
  {#if items.length === 0}
    <div class="px-3 py-2 text-xs text-base-content/50">
      {t("agent.slash.noMatch")}
    </div>
  {:else}
    {#each items as skill, index (skill.name)}
      {@const active = index === highlightedIndex}
      <button
        type="button"
        role="option"
        aria-selected={active}
        class={`flex w-full flex-col gap-0.5 px-3 py-1.5 text-left transition-colors ${
          active ? "bg-info/15 text-info" : "text-base-content/80 hover:bg-base-300"
        }`}
        onmousedown={(event) => {
          // mousedown（非 click）以免 textarea 先失焦丢掉选区/触发关闭。
          event.preventDefault();
          onSelect(skill);
        }}
        onmouseenter={() => onHover(index)}
      >
        <span class="truncate text-sm font-medium">{skill.name}</span>
        {#if skill.description}
          <span class="truncate text-xs text-base-content/50">
            {skill.description}
          </span>
        {/if}
      </button>
    {/each}
  {/if}
</div>
