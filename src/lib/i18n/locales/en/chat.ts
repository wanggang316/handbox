/**
 * chat namespace strings (filled by migration subagent).
 */
import type { chatZh } from "../zh/chat";

export const chatEn: Record<keyof typeof chatZh, string> = {
  // Empty state / loading
  "chat.loadingMessages": "Loading messages...",
  "chat.startNewConversation": "Start a new conversation",

  // Delete / resend / regenerate confirmation dialogs
  "chat.deleteConfirmTitle": "Confirm deletion",
  "chat.deleteConfirmMessage": "Are you sure you want to delete this message?",
  "chat.resendConfirmTitle": "Confirm resend",
  "chat.resendConfirmMessage":
    "Resending this message will delete all messages after it. Continue?",
  "chat.regenerateConfirmTitle": "Confirm regeneration",
  "chat.regenerateConfirmMessage":
    "Regenerating this reply will delete this message and all messages after it. Continue?",
  "chat.resend": "Resend",
  "chat.regenerate": "Regenerate",

  // Input box
  "chat.editMessage": "Edit message",
  "chat.cancelEdit": "Cancel editing",
  "chat.editMessagePlaceholder": "Edit message content...",
  "chat.inputPlaceholder": "Type a message here, press Enter to send",
  "chat.addAttachment": "Add attachment",
  "chat.uploadImage": "Upload image",
  "chat.removeImage": "Remove image",
  "chat.updateMessage": "Update message",

  // Model selection
  "chat.selectModel": "Select model",
  "chat.searchModelPlaceholder": "Search models...",
  "chat.loadingModels": "Loading models...",
  "chat.modelCount": "{count} models found",
  "chat.allProviders": "All providers",
  "chat.favorites": "Favorites",
  "chat.noMatchingModels": "No matching models found",
  "chat.adjustSearchHint": "Try adjusting your search or clearing the filters",
  "chat.supportsImageGeneration": "Supports image generation",
  "chat.contextLength": "Context length",
  "chat.maxOutputLength": "Max output length",
  "chat.inputPrice": "Input price",
  "chat.outputPrice": "Output price",
  "chat.modelProvider": "Model provider",
  "chat.noModelSelected": "No model selected",
  "chat.selectModelToStart": "Select a model to start the conversation",

  // Chat header / settings
  "chat.settings": "Settings",
  "chat.chatSettings": "Chat settings",
  "chat.advanced": "Advanced",

  // System Prompt
  "chat.noSystemPrompt": "No system prompt yet",
  "chat.editSystemPrompt": "Edit system prompt",
  "chat.systemPromptPlaceholder": "Enter system prompt...",
  "chat.characterCount": "Characters: {count}",

  // Reasoning / Thinking
  "chat.followModel": "Follow model",
  "chat.effort": "Effort",
  "chat.summary": "Summary",
  "chat.includeReasoning": "Include reasoning",
  "chat.includeThoughts": "Include thoughts",
  "chat.budgetMode": "Budget mode",

  // Tools / MCP
  "chat.tools": "Tools",
  "chat.autoExecution": "Auto",
  "chat.manualExecution": "Manual",
  "chat.selectOrCreateChatFirst": "Please select or create a chat first",
  "chat.serversSelected": "{count} servers selected",
  "chat.mcpAssociatedWithChat": "MCP server configuration is tied to the chat",
  "chat.noAvailableMcpServers": "No available MCP servers",
  "chat.configureMcpInSettings":
    "Configure and enable MCP servers in the app settings",
  "chat.enabledToolsCount": "{count} enabled tools",
  "chat.disabledServersHeading": "Disabled servers ({count})",
  "chat.serverDisabled": "● Server disabled",
  "chat.serverNotReady": "● Server not ready",
  "chat.serverDeleted": "● Server deleted",
  "chat.serverDisabledHint":
    "This server is disabled in the global settings; enable it before use",
  "chat.serverNotReadyHint":
    "This server is in an abnormal state; check its configuration",
  "chat.serverDeletedHint":
    "This server has been deleted; we recommend removing this configuration",

  // Message actions
  "chat.copyMessage": "Copy message",
  "chat.editAndResend": "Edit and resend",
  "chat.resendMessage": "Resend message",
  "chat.deleteMessage": "Delete message",
  "chat.openInSystemPreview": "Click to open in system preview",

  // Assistant message
  "chat.reasoningInProgress": "Reasoning...",
  "chat.reasoningProcess": "Reasoning",
  "chat.generatingImage": "Generating image…",
  "chat.copyImage": "Copy image",
  "chat.saveImage": "Save image",
  "chat.openInFinder": "Reveal in Finder",

  // Tool calls
  "chat.toolPending": "Pending",
  "chat.toolExecuting": "Executing",
  "chat.toolCompleted": "Completed",
  "chat.toolFailed": "Failed",
  "chat.toolUnknown": "Unknown",
  "chat.toolFallbackName": "Tool {index}",
  "chat.execute": "Execute",
  "chat.reExecute": "Re-execute",

  // Page-level prompts / title generation
  "chat.titleGenerationFailed":
    "Auto title generation failed; right-click the chat to generate manually",
  "chat.reasonServerDisabled": "Server disabled",
  "chat.reasonServerNotReady": "Server not ready",
  "chat.reasonServerDeleted": "Server deleted",
  "chat.disabledMcpDetectedTitle": "Disabled MCP servers detected",
  "chat.disabledMcpDetectedMessage":
    "The current chat configuration has <span class='font-medium'>{count}</span> MCP server(s) disabled in the global settings:<br/>{list}",
};
