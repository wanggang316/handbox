/**
 * agent namespace strings (filled by migration subagent).
 */
import type { agentZh } from "../zh/agent";

export const agentEn: Record<keyof typeof agentZh, string> = {
  // System Prompt popover (AgentSessionHeader)
  "agent.systemPrompt.editAria": "Edit System Prompt",
  "agent.systemPrompt.placeholder": "Enter a system prompt...",
  "agent.systemPrompt.saveFailed": "Save failed: {error}",

  // Thinking-level selector (AgentInput)
  "agent.thinking.off": "Off",
  "agent.thinking.low": "Low",
  "agent.thinking.medium": "Medium",
  "agent.thinking.high": "High",

  // Input composer (AgentInput)
  "agent.input.oversizeSkipped": "Some images over 10MB were skipped",
  "agent.input.steerFailed": "Failed to send steering message",
  "agent.input.selectModelFirst": "Select a model first",
  "agent.input.runFailed": "Failed to start the agent run",
  "agent.input.removeImage": "Remove image",
  "agent.input.awaitingApprovalPlaceholder":
    "Awaiting approval — allow or deny in the dialog",
  "agent.input.placeholder": "Type a message, press Enter to send",
  "agent.input.awaitingApprovalHint":
    "Awaiting tool approval, conversation paused",
  "agent.input.addImage": "Add image",
  "agent.input.uploadImage": "Upload image",
  "agent.input.tools": "Tools",
  "agent.input.workingDirRequired": "Set a working directory to enable tools",
  "agent.input.toolNeedsWorkingDir": "{label} (needs a working directory)",
  "agent.input.stop": "Stop",
  "agent.input.send": "Send",

  // Timeline (AgentTimeline)
  "agent.timeline.compacting": "Compacting context…",
  "agent.timeline.usageInput": "Input {count}",
  "agent.timeline.usageOutput": "Output {count}",

  // Thinking block (AgentThinkingBlock)
  "agent.thinkingBlock.streaming": "Thinking...",
  "agent.thinkingBlock.title": "Thinking",

  // Built-in tool labels (constants/agentTools.ts; shared by settings + AgentInput).
  // Kept identical to the coding-agent registration id so the UI reads the same
  // name the backend gates on — no per-language alias to mentally map back.
  "agent.tool.read": "read",
  "agent.tool.write": "write",
  "agent.tool.edit": "edit",
  "agent.tool.bash": "bash",
  "agent.tool.grep": "grep",
  "agent.tool.find": "find",
  "agent.tool.ls": "ls",

  // Tool-call card (AgentToolCallCard)
  "agent.toolCall.executing": "Running",
  "agent.toolCall.completed": "Done",
  "agent.toolCall.error": "Failed",
  "agent.toolCall.fallbackName": "Tool",
  "agent.toolCall.resultImageAlt": "Tool result image",

  // Approval modal (AgentApprovalModal)
  "agent.approval.toolWrite": "Write file",
  "agent.approval.toolEdit": "Edit file",
  "agent.approval.toolBash": "Run command",
  "agent.approval.toolFallback": "Tool call",
  "agent.approval.title": "Your confirmation is required",
  "agent.approval.intro":
    "Agent wants to perform the following action and will only run it after you confirm. Please review the parameters.",
  "agent.approval.command": "Command",
  "agent.approval.targetPath": "Target path",
  "agent.approval.content": "Content",
  "agent.approval.fullArgs": "Full parameters",
  "agent.approval.deny": "Deny",
  "agent.approval.allowOnce": "Allow once",
  "agent.approval.allowAlways": "Always allow",

  // Skill slash popover (SkillSlashPopover)
  "agent.slash.ariaLabel": "Skill autocomplete",
  "agent.slash.noMatch": "No matching skill",

  // Project / session list (AgentProjectList)
  "agent.list.renamePlaceholder": "Enter a new name",
  "agent.list.heading": "Agent Sessions",
  "agent.list.pickProjectDir": "Choose a project directory",
  "agent.list.loadFailed": "Failed to load list",
  "agent.list.emptyHint": "Click + to choose a project directory and start",
  "agent.list.noChats": "No chats",
  "agent.list.ungrouped": "Ungrouped",
  "agent.list.newSession": "New session",
  "agent.list.newSessionInProject": "New session in project {name}",
  "agent.list.copyPath": "Copy path",
  "agent.list.deleteProject": "Delete project",
  "agent.list.copyId": "Copy ID",
  "agent.list.untitledSession": "Untitled",
  "agent.list.deleteProjectConfirm":
    "This will delete project “{name}” and its {count} session(s) permanently.",
  "agent.list.deleteProjectFailed": "Failed to delete project",
  "agent.list.createProjectFailed": "Failed to create project",
  "agent.list.createSessionFailed": "Failed to create session",

  // Agent form modal (AgentFormModal)
  "agent.form.nameRequired": "Enter an Agent name",
  "agent.form.saveFailed": "Save failed, please try again",
  "agent.form.editTitle": "Edit Agent",
  "agent.form.createTitle": "New Agent",
  "agent.form.nameLabel": "Name",
  "agent.form.namePlaceholder": "Enter an Agent name",
  "agent.form.modelLabel": "Model",
  "agent.form.modelPlaceholder":
    "Enter a model identifier (e.g. gpt-4, claude-3-5-sonnet-20241022)",
  "agent.form.modelHint":
    "The model identifier can be any string, not limited to configured models",
  "agent.form.systemPromptTitle": "System prompt",
  "agent.form.charCount": "{count} chars",
  "agent.form.skillsTitle": "Skills",
  "agent.form.skillsLabel": "Skill tags",
  "agent.form.skillsPlaceholder": "e.g. coding, writing, translation",
  "agent.form.skillsHint": "Separate multiple skill tags with commas",
  "agent.form.modelParams": "Model parameters",
  "agent.form.mcpServers": "MCP servers",
  "agent.form.mcpComingSoon": "MCP server configuration is coming soon...",

  // Agent session landing page (agent/+page.svelte)
  "agent.page.startConversation": "Start a conversation with {name}",
  "agent.page.landingWithProjects":
    "Pick a session on the left, or click + on a project to create one",
  "agent.page.landingNoProjects":
    "Click + on the left to choose a project directory",

  // Agents management page (agents/+page.svelte)
  "agent.manage.count": "{count} total",
  "agent.manage.newAgent": "New Agent",
  "agent.manage.searchPlaceholder": "Search Agent name or skills...",
  "agent.manage.noMatch": "No matching Agent found",
  "agent.manage.clearSearch": "Clear search",
  "agent.manage.empty": "No Agents created yet",
  "agent.manage.emptyHint": "Click the button above to create your first Agent",
  "agent.manage.use": "Use",
  "agent.manage.modelUnset": "Not set",
  "agent.manage.deleteTitle": "Delete Agent",
  "agent.manage.deleteConfirm":
    "Are you sure you want to delete this Agent? This action cannot be undone.",
};
