<script lang="ts">
  import { ShieldAlert, FilePlus, FilePen, Terminal } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import { t } from "$lib/i18n";
  import type {
    AgentApprovalRequest,
    ApprovalDecision,
  } from "$lib/types/agentSession";

  interface Props {
    // Pending approval request; args are the exact parameters about to run and
    // must be shown in full.
    request: AgentApprovalRequest;
    // Decision callback. It passes back the request THIS modal displays, so the
    // caller responds to that exact requestId (shown == answered, no re-fetch
    // race). "allow_once" runs the tool this time only; "allow_always" also
    // skips future prompts for the same tool in this session; "deny" cancels
    // the tool and the conversation continues.
    onRespond: (
      request: AgentApprovalRequest,
      decision: ApprovalDecision,
    ) => void;
  }

  let { request, onRespond }: Props = $props();

  // Tool name → localized label + icon. Unknown names fall back to the raw
  // toolName (never silently hide the call). $derived so labels track language switch.
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

  // Defensive string read from args (shape is dictated by the backend tool schema).
  function argString(key: string): string | null {
    if (!request.args || typeof request.args !== "object") return null;
    const value = (request.args as Record<string, unknown>)[key];
    return typeof value === "string" ? value : null;
  }

  // Full bash command (security-critical: never truncated into harmlessness).
  const command = $derived(
    request.toolName === "bash" ? argString("command") : null,
  );
  // Target path for write/edit (security-critical: shown in full). Schema keys
  // differ: write → `path`, edit → `file_path` (coding-agent tools/{write,edit}.rs).
  const targetPath = $derived.by(() => {
    if (request.toolName === "write") return argString("path");
    if (request.toolName === "edit") return argString("file_path");
    return null;
  });
  // Content preview for write/edit. Keys: write → `content`; single edit →
  // `new_string`; multi-edit → `edits: [{oldText, newText}]`, newText values
  // joined in application order.
  const contentPreview = $derived.by(() => {
    if (request.toolName === "write") return argString("content");
    if (request.toolName === "edit") {
      const multi = editNewTextJoined();
      if (multi !== null) return multi;
      return argString("new_string");
    }
    return null;
  });

  // Join `args.edits[].newText` for the multi-edit shape; null when there is no
  // edits array (falls back to new_string). Only string newText values are taken.
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

  // Render the full args as formatted JSON: the shown-equals-executed fallback
  // view, so parameters not covered by the structured fields above stay visible.
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

  // Security invariant: args are LLM-controlled untrusted text injected via
  // `{@html}`. `renderCodeBlock` HTML-escapes the source before emitting token
  // markup, with an `escapeHtml` fallback on highlight failure ($lib/utils/code),
  // so injected `<img onerror=...>` renders as visible text, not live nodes.
  // Never replace this with unescaped innerHTML concatenation.
  function renderText(text: string): string {
    return renderCodeBlock(text, { variant: "compact" });
  }

  // Closing means deny (fail-closed): `Modal` routes Escape to `onClose`.
  // The modal must never close without a decision — the backend oneshot would
  // keep awaiting and the conversation would stay paused. So every close path
  // is treated as "deny". The store is idempotent per requestId, so racing
  // with a button click only lets the first decision win.
  function handleClose(): void {
    onRespond(request, "deny");
  }

  // Focus trapping comes from `Modal` (bits-ui Dialog FocusScope): while a request
  // is pending, Tab cycles inside the dialog and never reaches the disabled input
  // behind it. Focus opens on the dialog container (Modal suppresses first-element
  // autofocus), so Enter cannot hit a button by accident and Escape always denies.
</script>

<Modal open={true} showCloseButton={false} onClose={handleClose}>
  <!-- aria-labelledby ties the content to the title so screen readers announce
       a named modal dialog. -->
  <div
    aria-labelledby="agent-approval-title"
    class="w-[560px] max-w-[90vw] flex flex-col"
  >
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

      <!-- Full bash command, untruncated. -->
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

      <!-- write/edit target path, shown in full. -->
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

      <!-- write/edit content preview, scrollable when long. -->
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

      <!-- Full args JSON: the shown-equals-executed fallback view. -->
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

    <!-- Deny / allow-once / allow-always. "Always" is remembered per tool per
         session in backend process memory only — not across sessions/restarts. -->
    <div
      class="flex items-center justify-end gap-3 px-6 pt-3 pb-4 border-t border-[var(--hairline)]"
    >
      <Button
        class="w-20"
        size="md"
        variant="secondary"
        onclick={() => onRespond(request, "deny")}
      >{t("agent.approval.deny")}</Button>
      <Button
        class="w-24"
        size="md"
        variant="secondary"
        onclick={() => onRespond(request, "allow_once")}
      >{t("agent.approval.allowOnce")}</Button>
      <Button
        class="w-28"
        size="md"
        variant="primary"
        onclick={() => onRespond(request, "allow_always")}
      >{t("agent.approval.allowAlways")}</Button>
    </div>
  </div>
</Modal>
