<script lang="ts">
  import { ShieldAlert, FilePlus, FilePen, Terminal } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import type { AgentApprovalRequest } from "$lib/types/agentSession";

  interface Props {
    // 当前待审批请求（非空即弹窗打开、对话暂停）。args 即将执行的参数，须完整呈现。
    request: AgentApprovalRequest;
    // 用户决策回调：true=允许（工具执行、对话继续）/ false=拒绝（工具被 Cancel）。
    onRespond: (allow: boolean) => void;
  }

  let { request, onRespond }: Props = $props();

  // 工具名 → 中文 label + 图标。危险工具（write/edit/bash）已知集；未知名兜底回显
  // 原始 toolName（绝不静默隐藏调用本体）。
  const TOOL_META: Record<string, { label: string; icon: typeof Terminal }> = {
    write: { label: "写入文件", icon: FilePlus },
    edit: { label: "编辑文件", icon: FilePen },
    bash: { label: "执行命令", icon: Terminal },
  };

  const meta = $derived(
    TOOL_META[request.toolName] ?? {
      label: request.toolName || "工具调用",
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
  // write/edit 的目标路径（安全关键：必须完整可见）。
  const targetPath = $derived(
    request.toolName === "write" || request.toolName === "edit"
      ? argString("path")
      : null,
  );
  // write/edit 的内容预览（content / new_string 等；长内容可滚动）。
  const contentPreview = $derived.by(() => {
    if (request.toolName === "write") return argString("content");
    if (request.toolName === "edit") {
      // edit 的写入参数命名可能为 new_string / new；两者皆取，优先 new_string。
      return argString("new_string") ?? argString("new");
    }
    return null;
  });

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

  function renderText(text: string): string {
    return renderCodeBlock(text, { variant: "compact" });
  }
</script>

<Modal open={true} showCloseButton={false}>
  <div class="w-[560px] max-w-[90vw] flex flex-col">
    <!-- 头部：危险图标 + 标题。 -->
    <div
      class="flex items-center gap-2 px-6 pt-5 pb-3 border-b border-[var(--hairline)]"
    >
      <span class="text-warning">
        <ShieldAlert size={18} />
      </span>
      <h2 class="text-sm font-medium text-base-content">需要你的确认</h2>
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
        Agent 请求执行以下操作，确认后才会运行。请核对参数。
      </p>

      <!-- bash：完整 command（不截断）。 -->
      {#if command !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">命令</div>
          <div class="text-[11px] break-words leading-relaxed">
            {@html renderText(command)}
          </div>
        </div>
      {/if}

      <!-- write/edit：目标路径（完整可见）。 -->
      {#if targetPath !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">目标路径</div>
          <div class="text-[11px] break-all leading-relaxed">
            {@html renderText(targetPath)}
          </div>
        </div>
      {/if}

      <!-- write/edit：内容预览（长内容可滚动）。 -->
      {#if contentPreview !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/60">内容</div>
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
          <div class="mb-1 text-[10px] text-base-content/60">完整参数</div>
          <div class="text-[11px] break-words leading-relaxed">
            {@html argsJson}
          </div>
        </div>
      {/if}
    </div>

    <!-- 底部：拒绝 / 允许。 -->
    <div
      class="flex items-center justify-end gap-3 px-6 pt-3 pb-4 border-t border-[var(--hairline)]"
    >
      <RoundButton
        customClass="w-24"
        label="拒绝"
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-base-300"
        textColor="text-base-content/80"
        hoverColor="hover:bg-base-300/80"
        onclick={() => onRespond(false)}
      />
      <RoundButton
        customClass="w-24"
        label="允许"
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-primary"
        textColor="text-primary-content"
        hoverColor="hover:bg-primary/90"
        onclick={() => onRespond(true)}
      />
    </div>
  </div>
</Modal>
