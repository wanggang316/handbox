<script lang="ts">
  import { ShieldAlert, FilePlus, FilePen, Terminal } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import { t } from "$lib/i18n";
  import type {
    AgentApprovalRequest,
    ApprovalDecision,
  } from "$lib/types/agentSession";

  interface Props {
    // 当前待审批请求（非空即弹窗打开、对话暂停）。args 即将执行的参数，须完整呈现。
    request: AgentApprovalRequest;
    // 用户决策回调（含作用域）：回调透传**本弹窗当前展示的 request**，使调用方据其
    // requestId 精确回灌（展示==回灌，无 sessionId 重取竞态）：
    //  - "allow_once"   本次允许（工具执行、对话继续；同工具下次仍弹窗）；
    //  - "allow_always" 始终允许该工具（本会话）—— 同会话同工具后续不再弹窗、直接执行；
    //  - "deny"         拒绝（工具被 Cancel、模型收被拒结果、对话继续不中断）。
    onRespond: (
      request: AgentApprovalRequest,
      decision: ApprovalDecision,
    ) => void;
  }

  let { request, onRespond }: Props = $props();

  // 工具名 → 本地化 label + 图标。危险工具（write/edit/bash）已知集；未知名兜底回显
  // 原始 toolName（绝不静默隐藏调用本体）。$derived so labels track language switch.
  const TOOL_META = $derived<
    Record<string, { label: string; icon: typeof Terminal }>
  >({
    write: { label: t("agent.approval.toolWrite"), icon: FilePlus },
    edit: { label: t("agent.approval.toolEdit"), icon: FilePen },
    bash: { label: t("agent.approval.toolBash"), icon: Terminal },
  });

  const meta = $derived(
    TOOL_META[request.toolName] ?? {
      label: request.toolName || t("agent.approval.toolFallback"),
      icon: ShieldAlert,
    },
  );
  const ToolIcon = $derived(meta.icon);

  // 从 args 中安全取字符串字段（args 形状由后端工具 schema 决定，此处防御性读取）。
  function argString(key: string): string | null {
    if (!request.args || typeof request.args !== "object") return null;
    const value = (request.args as Record<string, unknown>)[key];
    return typeof value === "string" ? value : null;
  }

  // bash 的完整 command（安全关键：必须完整可见，不截断到看不出危险性）。
  const command = $derived(
    request.toolName === "bash" ? argString("command") : null,
  );
  // write/edit 的目标路径（安全关键：必须完整可见）。后端 schema 键各异：
  // write → `path`，edit → `file_path`（见 coding-agent tools/{write,edit}.rs）。
  const targetPath = $derived.by(() => {
    if (request.toolName === "write") return argString("path");
    if (request.toolName === "edit") return argString("file_path");
    return null;
  });
  // write/edit 的内容预览（长内容可滚动）。各工具真实键：
  //  - write → `content`；
  //  - edit 单编辑 → `new_string`；
  //  - edit 多编辑 → `edits: [{oldText, newText}]`，拼接各 `newText`（顺序即应用序）。
  const contentPreview = $derived.by(() => {
    if (request.toolName === "write") return argString("content");
    if (request.toolName === "edit") {
      const multi = editNewTextJoined();
      if (multi !== null) return multi;
      return argString("new_string");
    }
    return null;
  });

  // edit 多编辑 shape：从 `args.edits[].newText` 拼出待写入内容（无 edits 数组返回
  // null，回落到单编辑 new_string）。防御性读取：仅取字符串 newText，跳过畸形项。
  function editNewTextJoined(): string | null {
    if (!request.args || typeof request.args !== "object") return null;
    const edits = (request.args as Record<string, unknown>).edits;
    if (!Array.isArray(edits)) return null;
    const parts = edits
      .map((entry) =>
        entry && typeof entry === "object"
          ? (entry as Record<string, unknown>).newText
          : undefined,
      )
      .filter((v): v is string => typeof v === "string");
    return parts.length > 0 ? parts.join("\n") : null;
  }

  // 把完整 args（任意结构）渲染为格式化 JSON 代码块，作为「展示值==执行值」的兜底
  // 全量视图——即便上面的结构化字段未覆盖某工具的某参数，完整参数仍在此可见
  // （VAL-CAPERM-002）。镜像 AgentToolCallCard 的 renderArgs 风格。
  const argsJson = $derived.by(() => {
    if (request.args === undefined || request.args === null) return "";
    let formatted: string;
    if (typeof request.args === "string") {
      try {
        formatted = JSON.stringify(JSON.parse(request.args), null, 2);
      } catch {
        formatted = request.args;
      }
    } else {
      formatted = JSON.stringify(request.args, null, 2);
    }
    return renderCodeBlock(formatted, { language: "json", variant: "compact" });
  });

  // 安全前提（VAL-CAPERM-002 知情同意）：args 是 LLM 控制的不可信文本，经 `{@html}`
  // 注入 DOM。`renderCodeBlock`/`renderText` 经 highlight.js（`highlightAuto` 或显式
  // 语言）对源文本做 HTML 转义后再返回 token 标记，highlight 异常时也走 `escapeHtml`
  // 兜底（见 $lib/utils/code）——故模型在 args 注入的 `<img onerror=...>` 等被渲染为
  // 可见文本而非可执行节点，弹窗不会执行注入脚本。切勿改为未经转义的 innerHTML 拼接。
  function renderText(text: string): string {
    return renderCodeBlock(text, { variant: "compact" });
  }

  // 关闭路径 == 拒绝（fail-closed，VAL-CAPERM-015）：`Modal` 把 Escape 键接到
  // `onClose`。审批弹窗绝不能被「无决策地关掉」——那样后端 oneshot 仍在 await、对话
  // 卡在暂停态。任何关闭路径（这里是 Escape）都按 `deny` 处理：工具被 Cancel、模型
  // 收被拒结果、对话继续不中断（与点「拒绝」按钮同义）。store 对重复/未知 requestId
  // 幂等，故即便和按钮点击竞合也只首处置生效。
  function handleClose(): void {
    onRespond(request, "deny");
  }

  // 焦点陷阱（VAL-CAPERM-021）：审批是安全关键的强制决策点，待决期间键盘焦点必须
  // 困在弹窗内——Tab/Shift+Tab 在弹窗内的可聚焦控件间循环，绝不跳到背后已禁用的
  // 输入区/发送按钮。共享 `Modal` 只设置了「打开即聚焦 backdrop」的初始焦点，未做
  // Tab 循环，故在此对审批内容补焦点陷阱（不改共享组件）。初始焦点落在第一个动作
  // 按钮（拒绝，最安全的默认），用户无需移动鼠标即可键盘操作。
  function trapFocus(node: HTMLElement) {
    const FOCUSABLE =
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

    function focusable(): HTMLElement[] {
      return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      );
    }

    function onKeydown(event: KeyboardEvent) {
      if (event.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) {
        // 无可聚焦项：吞掉 Tab 以免焦点逃逸到背后的输入区。
        event.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (event.shiftKey) {
        if (active === first || !node.contains(active)) {
          event.preventDefault();
          last.focus();
        }
      } else if (active === last || !node.contains(active)) {
        event.preventDefault();
        first.focus();
      }
    }

    node.addEventListener("keydown", onKeydown);
    // 初始焦点：第一个动作按钮（拒绝）。tick 由 action 挂载时机保证 DOM 已就绪。
    focusable()[0]?.focus();

    return {
      destroy() {
        node.removeEventListener("keydown", onKeydown);
      },
    };
  }
</script>

<Modal open={true} showCloseButton={false} onClose={handleClose}>
  <!--
    审批内容根：挂焦点陷阱（VAL-CAPERM-021），待决期间 Tab 在弹窗内循环、绝不跳到
    背后已禁用的输入区/发送。`aria-labelledby` 把内容关联到标题，配合外层 `Modal` 的
    role="dialog"/aria-modal 让屏幕阅读器把它读作一个有名字的模态对话框。
  -->
  <div
    use:trapFocus
    aria-labelledby="agent-approval-title"
    class="w-[560px] max-w-[90vw] flex flex-col"
  >
    <!-- 头部：危险图标 + 标题。 -->
    <div
      class="flex items-center gap-2 px-6 pt-5 pb-3 border-b border-[var(--hairline)]"
    >
      <span class="text-warning">
        <ShieldAlert size={18} />
      </span>
      <h2 id="agent-approval-title" class="text-sm font-medium text-base-content">
        {t("agent.approval.title")}
      </h2>
    </div>

    <!-- 内容：工具名 + 完整参数（路径 / 命令完整可见，长内容可滚动）。 -->
    <div class="px-6 py-4 space-y-3 max-h-[60vh] overflow-y-auto">
      <div class="flex items-center gap-2 text-base-content">
        <ToolIcon size={16} class="shrink-0 text-warning" />
        <span class="text-sm font-medium">{meta.label}</span>
        <span class="text-[11px] text-base-content/50">({request.toolName})</span
        >
      </div>

      <p class="text-[12px] text-base-content/70">
        {t("agent.approval.intro")}
      </p>

      <!-- bash：完整 command（不截断）。 -->
      {#if command !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">
            {t("agent.approval.command")}
          </div>
          <div class="text-[11px] break-words leading-relaxed">
            {@html renderText(command)}
          </div>
        </div>
      {/if}

      <!-- write/edit：目标路径（完整可见）。 -->
      {#if targetPath !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">
            {t("agent.approval.targetPath")}
          </div>
          <div class="text-[11px] break-all leading-relaxed">
            {@html renderText(targetPath)}
          </div>
        </div>
      {/if}

      <!-- write/edit：内容预览（长内容可滚动）。 -->
      {#if contentPreview !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">
            {t("agent.approval.content")}
          </div>
          <div
            class="text-[11px] break-words leading-relaxed max-h-48 overflow-auto"
          >
            {@html renderText(contentPreview)}
          </div>
        </div>
      {/if}

      <!-- 完整参数（JSON）：展示值==执行值的兜底全量视图。 -->
      {#if argsJson}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">
            {t("agent.approval.fullArgs")}
          </div>
          <div class="text-[11px] break-words leading-relaxed">
            {@html argsJson}
          </div>
        </div>
      {/if}
    </div>

    <!--
      底部：拒绝 / 本次允许 / 始终允许（本会话）。允许拆成两个可见选项——「本次允许」
      一次性（同工具下次仍弹窗）、「始终允许」本会话记住该工具（同会话同工具后续不再
      弹窗、直接执行；后端进程内存集、不跨会话/重启）。
    -->
    <div
      class="flex items-center justify-end gap-3 px-6 pt-3 pb-4 border-t border-[var(--hairline)]"
    >
      <RoundButton
        customClass="w-20"
        label={t("agent.approval.deny")}
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-base-300"
        textColor="text-base-content/80"
        hoverColor="hover:bg-base-300/80"
        onclick={() => onRespond(request, "deny")}
      />
      <RoundButton
        customClass="w-24"
        label={t("agent.approval.allowOnce")}
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-base-300"
        textColor="text-base-content/80"
        hoverColor="hover:bg-base-300/80"
        onclick={() => onRespond(request, "allow_once")}
      />
      <RoundButton
        customClass="w-28"
        label={t("agent.approval.allowAlways")}
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-primary"
        textColor="text-primary-content"
        hoverColor="hover:bg-primary/90"
        onclick={() => onRespond(request, "allow_always")}
      />
    </div>
  </div>
</Modal>
