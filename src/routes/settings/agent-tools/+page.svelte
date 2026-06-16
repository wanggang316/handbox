<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { BUILTIN_TOOLS, BUILTIN_TOOL_IDS } from "$lib/constants/agentTools";

  // 全局默认启用的工具集（coding-agent 注册名）。无 agent 段时视为全开默认。
  let enabledTools = $state<string[]>([...BUILTIN_TOOL_IDS]);

  onMount(async () => {
    try {
      await settingsState.loadSettings();
      enabledTools =
        settingsState.settings?.agent?.defaultEnabledTools ?? [
          ...BUILTIN_TOOL_IDS,
        ];
    } catch (error) {
      console.error("加载 Agent 工具设置失败:", error);
    }
  });

  function isEnabled(toolId: string): boolean {
    return enabledTools.includes(toolId);
  }

  async function handleToggle(toolId: string, checked: boolean) {
    const next = checked
      ? enabledTools.includes(toolId)
        ? enabledTools
        : [...enabledTools, toolId]
      : enabledTools.filter((id) => id !== toolId);
    enabledTools = next;
    try {
      await settingsState.updateSettings({
        section: "agent",
        data: { defaultEnabledTools: next },
      });
    } catch (error) {
      console.error("更新 Agent 工具设置失败:", error);
    }
  }
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <div class="flex flex-col gap-y-1">
    <h2 class="text-base font-medium text-base-content">Agent 工具</h2>
    <p class="text-sm text-base-content/60">
      新建 Agent 会话默认启用的工具。已存在的会话不受影响。
    </p>
  </div>

  <TableGroup>
    {#each BUILTIN_TOOLS as tool (tool.id)}
      <SwitchRow
        label={tool.label}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
      />
    {/each}
  </TableGroup>
</div>
