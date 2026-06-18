/**
 * provider namespace strings (filled by migration subagent).
 */
import type { providerZh } from "../zh/provider";

export const providerEn: Record<keyof typeof providerZh, string> = {
  // Provider list page
  "provider.loadingProviders": "Loading providers...",
  "provider.emptyHint": "Add an AI provider to start using its models",
  "provider.addProvider": "Add Provider",
  "provider.addOtherProvider": "Add Another Provider",

  // Provider detail page
  "provider.modelList": "Models",
  "provider.addModel": "Add Model",
  "provider.addModelPlaceholder": "Enter a model id, e.g. llama-3.1-8b",
  "provider.adding": "Adding…",
  "provider.add": "Add",
  "provider.addModelFailed": "Failed to add model",
  "provider.refreshModels": "Refresh model list",
  "provider.backAria": "Back",
  "provider.supportImage": "Supports image generation",
  "provider.removeFromFavorites": "Remove from favorites",
  "provider.addToFavorites": "Add to favorites",
  "provider.viewModelInfo": "View model info",
  "provider.emptyCustomModels":
    "This custom provider has no models yet. Click “Add Model” to manually add a model id supported by the endpoint",
  "provider.emptyModels": "No models found. Check the provider configuration or your network connection",

  // Delete / disable confirmation dialogs
  "provider.deleteProviderTitle": "Delete Provider",
  "provider.deleteProviderMessage": "Delete <span class='font-medium'>{name}</span>?",
  "provider.disableProviderTitle": "Disable Provider",
  "provider.disableProviderWithChats":
    "Detected <span class='font-medium'>{count}</span> chat(s) using <span class='font-medium'>{name}</span>.<br/><br/>Disabling this provider will leave those chats unable to use its models.<br/><br/>Are you sure you want to disable it?",
  "provider.disableProviderConfirm": "Disable <span class='font-medium'>{name}</span>?",
  "provider.disableModelTitle": "Disable Model",
  "provider.disableModelWithChats":
    "Detected <span class='font-medium'>{count}</span> chat(s) using model <span class='font-medium'>{name}</span>.<br/><br/>Disabling this model will leave those chats unable to use it.<br/><br/>Are you sure you want to disable it?",
  "provider.disableModelConfirm": "Disable model <span class='font-medium'>{name}</span>?",
  "provider.closeAction": "Disable",
  "provider.disableAction": "Disable",

  // Add / edit provider dialog
  "provider.editProviderTitle": "Edit Provider",
  "provider.addProviderTitle": "Add Provider",
  "provider.providerType": "Provider Type",
  "provider.providerName": "Provider Name",
  "provider.confirm": "Confirm",
  "provider.configErrorTitle": "Provider Configuration Error",
  "provider.operationFailed": "Operation failed, please try again later",
  "provider.validateName": "Please enter a provider name",
  "provider.validateBaseUrl": "Please enter the Base URL",
  "provider.validateApiKey": "Please enter the API Key",
  "provider.updateSuccess": "Provider updated successfully",
  "provider.createSuccess": "Provider created successfully",

  // Model info dialog
  "provider.modelInfo": "Model Info",
  "provider.emptyModelInfo": "No model info available",
  "provider.viewModelDetail": "View model details",
  "provider.copyModelId": "Copy model ID",
  "provider.modelId": "Model ID",
  "provider.contextLength": "Context Length",
  "provider.maxOutputLength": "Max Output Length",
  "provider.inputPrice": "Input Price",
  "provider.outputPrice": "Output Price",
  "provider.supportedFeatures": "Supported Features",
  "provider.inputModalities": "Input Modalities",
  "provider.outputModalities": "Output Modalities",
  "provider.supportedMethods": "Supported Methods",
  "provider.supportedParameters": "Supported Parameters",

  // MCP list page
  "provider.loadingMcpServers": "Loading MCP servers...",
  "provider.mcpEmptyHint": "Add an MCP server to extend the AI's capabilities",
  "provider.addMcpServer": "Add MCP Server",
  "provider.mcpToolsSummary": "{total} tools, {enabled} enabled",
  "provider.editAria": "Edit",

  // MCP disable confirmation dialog
  "provider.disableMcpTitle": "Disable MCP Server",
  "provider.disableMcpWithChats":
    "Detected <span class='font-medium'>{count}</span> chat(s) using <span class='font-medium'>{name}</span>.<br/><br/>Choose an action:",
  "provider.disableMcpConfirm": "Disable <span class='font-medium'>{name}</span>?",
  "provider.disableAndRemove": "Unlink and disable",
  "provider.disableMcpOnly": "Disable MCP only",

  // MCP detail page
  "provider.deleteMcpTitle": "Delete MCP Server",
  "provider.deleteMcpMessage":
    "Delete <span class='font-medium'>{name}</span>?<br/><br/>This action cannot be undone.",
  "provider.lastSync": "Last synced: {time}",
  "provider.tabTools": "Tools",
  "provider.tabPrompts": "Prompts",
  "provider.tabResources": "Resources",
  "provider.emptyTools": "No tools available",
  "provider.emptyPrompts": "No prompts available",
  "provider.emptyResources": "No resources available",
  "provider.params": "Parameters",
  "provider.paramsWithCount": "Parameters ({count})",
  "provider.detail": "Details",

  // MCP form dialog
  "provider.editMcpTitle": "Edit MCP Server",
  "provider.addMcpTitle": "Add MCP Server",
  "provider.mcpName": "Name",
  "provider.mcpNamePlaceholder": "Unique name, e.g. filesystem",
  "provider.mcpDisplayName": "Display Name",
  "provider.mcpDisplayNamePlaceholder": "Optional human-readable name",
  "provider.connectionType": "Connection Type",
  "provider.connectionStdio": "Standard I/O (stdio)",
  "provider.connectionSse": "Server-Sent Events (SSE)",
  "provider.connectionHttp": "Streamable HTTP",
  "provider.mcpCommand": "Command",
  "provider.mcpCommandPlaceholder": "e.g. npx or uvx",
  "provider.mcpArgs": "Arguments",
  "provider.mcpArgsPlaceholder": "One per line, or comma-separated",
  "provider.mcpWorkingDir": "Working Directory",
  "provider.optional": "Optional",
  "provider.mcpEndpoint": "Endpoint URL",
  "provider.mcpEndpointPlaceholder": "e.g. http://localhost:3000/mcp or ws://localhost:8080",
  "provider.mcpTimeout": "Timeout (ms)",
  "provider.mcpTimeoutPlaceholder": "Optional, no timeout by default",
  "provider.envVars": "Environment Variables",
  "provider.addEntry": "Add",
  "provider.envKeyPlaceholder": "Key",
  "provider.envValuePlaceholder": "Value",
  "provider.httpHeaders": "HTTP Headers",
  "provider.headerKeyPlaceholder": "Header name",
  "provider.headerValuePlaceholder": "Header value",
  "provider.validateMcpName": "Please enter a server name",
  "provider.validateCommand": "Please enter a command to run",
  "provider.validateEndpoint": "Please enter the endpoint URL",
  "provider.validateTimeout": "Timeout must be a number",
  "provider.saveFailed": "Save failed, please try again",

  // MCP text edit dialog
  "provider.editMcpJsonTitle": "Edit MCP Server",
  "provider.mcpJsonPlaceholder": "Enter the MCP server configuration...",
  "provider.validateMcpJson": "Please enter the MCP server configuration",
};
