<script lang="ts">
  // Read-only summary of a sidebar session, shown to the right of its row on
  // hover. Purely informational: `pointer-events-none` keeps it out of the
  // hover chain, so it can never steal the hover that opened it.

  import { fly } from "svelte/transition";
  import { Folder, MessagesSquare, MonitorSmartphone } from "@lucide/svelte";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import { agentState } from "$lib/states/agent.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import { sessionActivityKey } from "$lib/utils/agentGrouping";
  import { formatRelativeTime } from "$lib/utils/date";
  import { t } from "$lib/i18n";
  import type { AgentSession } from "$lib/types";

  interface Props {
    session: AgentSession;
    /** Viewport coordinates of the card's top-left corner (already clamped). */
    x: number;
    y: number;
  }

  let { session, x, y }: Props = $props();

  // Resolved from the stores rather than passed down: the row renderer would
  // otherwise have to thread bucket/project through every call site, and a
  // dangling id must degrade to "no row" either way.
  const project = $derived(
    session.projectId
      ? agentProjectState.projects.find((p) => p.id === session.projectId)
      : undefined,
  );
  const agent = $derived(
    session.agentDefinitionId
      ? agentState.agents.find((a) => a.id === session.agentDefinitionId)
      : undefined,
  );
  const AgentIcon = $derived(resolveAgentIcon(agent?.icon));

  // A session with a working dir executes against real files on this machine;
  // one without is a plain dialog, which needs no such warning.
  const workingDir = $derived(session.workingDir);
</script>

<div
  class="fixed z-[var(--z-popover)] w-64 rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] px-3 py-2.5 shadow-xl pointer-events-none"
  style="left: {x}px; top: {y}px;"
  role="tooltip"
  transition:fly={{ x: -4, duration: 120 }}
>
  <div class="flex items-start gap-2">
    <span class="flex-1 text-[13px] leading-[18px] font-medium text-base-content break-words">
      {session.name || t("agent.list.untitledSession")}
    </span>
    <span class="flex-shrink-0 pt-px text-[11px] text-base-content/50">
      {formatRelativeTime(sessionActivityKey(session))}
    </span>
  </div>

  <div class="mt-2 space-y-1.5 text-[12px] leading-[16px] text-base-content/75">
    {#if project}
      <div class="flex items-center gap-2">
        <Folder size={14} class="flex-shrink-0 text-base-content/45" />
        <span class="truncate">{project.name}</span>
      </div>
    {/if}

    {#if agent}
      <div class="flex items-center gap-2">
        <AgentIcon size={14} class="flex-shrink-0 text-base-content/45" />
        <span class="truncate">{agent.name}</span>
      </div>
    {/if}

    {#if workingDir}
      <div class="flex items-center gap-2">
        <MonitorSmartphone size={14} class="flex-shrink-0 text-base-content/45" />
        <span class="truncate" title={workingDir}>{t("agent.list.card.localRun")}</span>
      </div>
    {/if}

    <div class="flex items-center gap-2">
      <MessagesSquare size={14} class="flex-shrink-0 text-base-content/45" />
      <span>{t("agent.list.card.messages", { count: session.messageCount })}</span>
    </div>
  </div>
</div>
