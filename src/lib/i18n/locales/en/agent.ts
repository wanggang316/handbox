/**
 * agent namespace strings.
 */
import type { agentZh } from "../zh/agent";

export const agentEn: Record<keyof typeof agentZh, string> = {
  // System Prompt popover (AgentSessionHeader)
  "agent.systemPrompt.editAria": "Edit System Prompt",
  "agent.systemPrompt.placeholder": "Enter a system prompt...",
  "agent.systemPrompt.saveFailed": "Save failed: {error}",

  // Thinking-level selector (AgentInput)
  "agent.thinking.label": "Reasoning effort",
  "agent.thinking.off": "Off",
  "agent.thinking.low": "Low",
  "agent.thinking.medium": "Medium",
  "agent.thinking.high": "High",
  "agent.thinking.offDesc": "No extended reasoning — fastest responses.",
  "agent.thinking.lowDesc":
    "Light reasoning — quick responses with brief thinking.",
  "agent.thinking.mediumDesc":
    "Medium reasoning — balances speed and reasoning depth.",
  "agent.thinking.highDesc":
    "Deep reasoning — thorough thinking for complex tasks.",

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
  "agent.input.awaitingQuestionPlaceholder":
    "Answer the questions above, or skip them",
  "agent.input.awaitingQuestionHint":
    "Awaiting your answer, conversation paused",
  "agent.input.addImage": "Add image",
  "agent.input.uploadImage": "Upload image",
  "agent.input.stop": "Stop",
  "agent.input.send": "Send",
  "agent.input.autoExecution": "Auto",
  "agent.input.manualExecution": "Manual",
  "agent.input.selectModel": "Select model",
  "agent.input.noAvailableMcpServers": "No available MCP servers",
  "agent.input.configureMcpInSettings":
    "Configure and enable MCP servers in the app settings",
  "agent.input.enabledToolsCount": "{count} enabled tools",
  "agent.input.selectAgent": "Select agent",
  "agent.input.switchAgentFailed": "Failed to switch agent",
  "agent.input.selectWorkingDir": "Select working directory",
  "agent.input.workingDirFailed": "Failed to set working directory",

  // Model select modal (ModelSelectModal)
  "agent.modelSelect.searchModelPlaceholder": "Search models...",
  "agent.modelSelect.loadingModels": "Loading models...",
  "agent.modelSelect.modelCount": "{count} models found",
  "agent.modelSelect.allProviders": "All providers",
  "agent.modelSelect.favorites": "Favorites",
  "agent.modelSelect.noMatchingModels": "No matching models found",
  "agent.modelSelect.adjustSearchHint":
    "Try adjusting your search or clearing the filters",
  "agent.modelSelect.supportsImageGeneration": "Supports image generation",
  "agent.modelSelect.contextLength": "Context length",
  "agent.modelSelect.maxOutputLength": "Max output length",
  "agent.modelSelect.inputPrice": "Input price",
  "agent.modelSelect.outputPrice": "Output price",

  // Timeline (AgentTimeline)
  "agent.timeline.compacting": "Compacting context…",
  "agent.timeline.usageInput": "Input {count}",
  "agent.timeline.usageOutput": "Output {count}",
  "agent.timeline.copy": "Copy",
  "agent.timeline.copied": "Copied",

  "agent.nav.label": "Message navigation",
  "agent.nav.jumpTo": "Jump to question {index}",
  "agent.nav.noAnswer": "No reply yet",
  "agent.timeline.genuiStreaming": "Generating UI…",

  // Thinking block (AgentThinkingBlock)
  "agent.thinkingBlock.streaming": "Thinking...",
  "agent.thinkingBlock.title": "Thinking",
  "agent.thinkingBlock.showMore": "Show more",
  "agent.thinkingBlock.showLess": "Show less",

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
  "agent.tool.web_search": "web_search",
  "agent.tool.render_card": "render_card",
  "agent.tool.render_app": "render_app",
  "agent.tool.ask_question": "ask_question",
  "agent.tool.skill": "skill",

  // Tool-call card (AgentToolCallCard)
  "agent.toolCall.executing": "Running",
  "agent.toolCall.completed": "Done",
  "agent.toolCall.error": "Failed",
  "agent.toolCall.fallbackName": "Tool",
  "agent.toolCall.resultImageAlt": "Tool result image",

  // Inline HTML card (HtmlCard)
  "agent.htmlCard.rendering": "Rendering card…",
  "agent.htmlCard.error": "Card failed to render",
  "agent.htmlCard.iframeTitle": "Interactive card",

  // HTML app pill + side panel (AppPill / AppPanel)
  "agent.htmlApp.generating": "Generating app…",
  "agent.htmlApp.error": "App failed to generate",
  "agent.htmlApp.untitled": "Untitled app",
  "agent.htmlApp.open": "View",
  "agent.htmlApp.preview": "Preview",
  "agent.htmlApp.code": "Code",
  "agent.htmlApp.close": "Close panel",
  "agent.htmlApp.iframeTitle": "App preview",

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

  // Question panel (AgentQuestionPanel; the ask_question tool)
  "agent.question.panelAria": "Agent questions",
  "agent.question.title": "The agent needs your input",
  "agent.question.progress": "{current} of {total}",
  "agent.question.prev": "Previous question",
  "agent.question.next": "Next question",
  "agent.question.goTo": "Go to question {index}",
  "agent.question.kindSingle": "Pick one",
  "agent.question.kindMultiple": "Pick any",
  "agent.question.kindText": "Write",
  "agent.question.textPlaceholder": "Type your answer…",
  "agent.question.required": "Required",
  "agent.question.submitBlocked": "Answer the required questions first",
  "agent.question.dismiss": "Skip and keep talking",
  "agent.question.submit": "Submit",

  // Skill slash popover (SkillSlashPopover)
  "agent.slash.ariaLabel": "Skill autocomplete",
  "agent.slash.noMatch": "No matching skill",

  // Project / session list (AgentProjectList)
  "agent.list.renamePlaceholder": "Enter a new name",
  "agent.list.heading": "Projects",
  "agent.list.pickProjectDir": "Choose a project directory",
  "agent.list.loadFailed": "Failed to load list",
  "agent.list.emptyHint": "No sessions yet — start one from Agents",
  "agent.list.noChats": "No chats",
  "agent.list.ungrouped": "Chats",
  "agent.list.newProject": "New project",
  "agent.list.newSession": "New session",
  "agent.list.newSessionInProject": "New session in project {name}",
  "agent.list.noProjectAgent": "No agent supports a working directory",
  "agent.list.moveToProject": "Move to project…",
  "agent.list.removeFromProject": "Remove from project",
  "agent.list.moveFailed": "Failed to move the session",
  "agent.list.copyPath": "Copy path",
  "agent.list.deleteProject": "Delete project",
  "agent.list.copyId": "Copy ID",
  "agent.list.untitledSession": "Untitled",

  // Session row hover actions + the Archived group
  "agent.list.pin": "Pin",
  "agent.list.unpin": "Unpin",
  "agent.list.archive": "Archive",
  "agent.list.unarchive": "Unarchive",
  "agent.list.archived": "Archived",
  "agent.list.pinFailed": "Failed to pin the session",
  "agent.list.archiveFailed": "Failed to archive the session",

  // Session hover card
  "agent.list.card.messages": "{count} messages",
  "agent.list.card.localRun": "Runs on your computer",

  "agent.list.deleteProjectConfirm":
    "This will delete project “{name}” and its {count} session(s) permanently.",
  "agent.list.deleteProjectFailed": "Failed to delete project",
  "agent.list.createProjectFailed": "Failed to create project",
  "agent.list.createSessionFailed": "Failed to create session",
  "agent.list.generateTitleFailed": "Failed to generate title",

  // Agent form modal (AgentFormModal)
  "agent.form.backToList": "Back to list",
  "agent.form.nameRequired": "Enter an Agent name",
  "agent.form.saveFailed": "Save failed, please try again",
  "agent.form.editTitle": "Edit Agent",
  "agent.form.createTitle": "New Agent",
  "agent.form.nameLabel": "Name",
  "agent.form.namePlaceholder": "Enter an Agent name",
  "agent.form.systemPromptTitle": "System prompt",
  "agent.form.charCount": "{count} chars",
  "agent.form.skillsTitle": "Skills",
  "agent.form.noSkills": "No skills available yet",
  "agent.form.searchSkills": "Search skills...",
  "agent.form.skillDisabled": "(globally disabled)",
  "agent.form.skillMissing": "(missing)",
  "agent.form.skillsLabel": "Skill tags",
  "agent.form.skillsPlaceholder": "e.g. coding, writing, translation",
  "agent.form.skillsHint": "Separate multiple skill tags with commas",
  "agent.form.modelParams": "Model parameters",
  "agent.form.mcpServers": "MCP servers",
  "agent.form.descriptionPlaceholder": "One-line summary shown in the list",
  "agent.form.iconLabel": "Icon",
  "agent.form.sectionTools": "Tools",
  "agent.form.sectionRuntime": "Runtime",
  "agent.form.linkedCount": "{count} linked",
  "agent.form.builtinTools": "Built-in tools",
  "agent.form.workingDir": "Working directory",
  "agent.form.toolExecution": "Tool execution",
  "agent.form.workingDirRequired": "Required",
  "agent.form.workingDirOptional": "Optional",
  "agent.form.workingDirNone": "None",
  "agent.form.generativeUi": "Generative UI",
  "agent.form.generativeUiDesc":
    "Let the assistant render interactive UI in replies",
  "agent.form.genuiHint": "Pick a saved template",
  "agent.form.genuiNone": "Not linked",
  "agent.form.mcpComingSoon": "MCP server configuration is coming soon...",

  // Agent session landing page (agent/+page.svelte)
  "agent.page.emptyGreeting": "What can I help you with today?",
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
  "agent.manage.loadFailed": "Failed to load agent",
  "agent.manage.deleteTitle": "Delete Agent",
  "agent.manage.deleteConfirm":
    "Are you sure you want to delete this Agent? This action cannot be undone.",
};
