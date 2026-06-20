// 消息服务实现

use crate::models::llm_types::{
    LlmGeneratedImage, LlmMessage, LlmMessageRole, LlmReasoningEffort, LlmToolFunction,
};
use crate::models::{
    AppError, MessageRequest, MessageRequestAttachment, MessageResponse, StreamChunk,
    UserMessageSendRequest,
};
use crate::services::chat_engine::{
    self, ChatMessage, ChatOptions, ChatProvider, ChatTool, ChatToolCall, HydratedAttachment,
};
use crate::services::{Database, McpService, ProviderService, SessionService, StorageService};
use crate::storage::types::{
    McpServer, McpServerStatus, Message, MessageAttachment, MessageConfig, MessageToolCall,
    MessageToolExecutionMode, MessageToolExecutionStatus, Session, UUID,
};
use crate::storage::MessageRepository;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::Utc;
use std::{collections::HashMap, fs, sync::Arc};
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;

/// 冻结的 "generative-UI" 系统提示词。
///
/// 由前端的 `buildGenerativeUiPrompt(uiCatalog)` 生成、经
/// `scripts/gen-generative-ui-prompt.ts` 写入 `resources/generative-ui-prompt.txt`，
/// 在编译期通过 `include_str!` 嵌入。一个 vitest drift 测试保证该 `.txt` 与生成器
/// 输出逐字节一致，因此此处嵌入的内容即权威目录提示。仅在会话
/// `generative_ui == Some(true)` 时注入到 LLM 请求的 System 段。
const GENERATIVE_UI_PROMPT: &str = include_str!("../../resources/generative-ui-prompt.txt");

/// 当会话关联了一份 GenUI 范例模板（`session.genui_spec`）时，把模板 spec 框定为
/// 「应模仿其结构与风格的输出范例」（few-shot / "A" 模式）。仅在 `generative_ui`
/// 开启且范例非空时，作为目录提示之后的又一条独立 System 段注入；单回合、不持久化。
const GENERATIVE_UI_EXAMPLE_PREAMBLE: &str = "以下是一份生成式 UI 输出范例。当你需要输出界面时，请模仿其组件结构与风格，仅把内容替换为与当前对话相关的真实信息：";

/// 流式回调 trait 定义
///
/// 这些 trait 用于统一流式消息处理的回调接口
///
/// # 回调执行顺序
/// 1. `StreamStartCallback` - 流式开始时触发
/// 2. `StreamingCallback` - 每次接收到数据块时触发（可多次）
/// 3. `StreamEndCallback` - 流式成功完成时触发
/// 4. `StreamErrorCallback` - 任何阶段发生错误时触发
///
/// 注意：`StreamEndCallback` 和 `StreamErrorCallback` 是互斥的，只会触发其中一个

/// 开始回调：当流式处理开始时调用
///
/// 参数:
/// - `stream_id`: 流式会话的唯一标识符
/// - `message_id`: 消息的唯一标识符
pub trait StreamStartCallback: FnMut(String, String) + Send + 'static {}
impl<T> StreamStartCallback for T where T: FnMut(String, String) + Send + 'static {}

/// 流式数据回调：每次接收到数据块时调用
///
/// 参数:
/// - `chunk`: 包含内容、推理过程和工具调用的数据块
pub trait StreamingCallback: FnMut(StreamChunk) + Send + 'static {}
impl<T> StreamingCallback for T where T: FnMut(StreamChunk) + Send + 'static {}

/// 结束回调：当流式处理成功完成时调用
///
/// 参数:
/// - `stream_id`: 流式会话的唯一标识符
/// - `response`: 完整的消息响应，包含最终内容、token统计等
pub trait StreamEndCallback: FnMut(String, MessageResponse) + Send + 'static {}
impl<T> StreamEndCallback for T where T: FnMut(String, MessageResponse) + Send + 'static {}

/// 错误回调：当流式处理发生错误时调用
///
/// 参数:
/// - `stream_id`: 流式会话的唯一标识符
/// - `error`: 应用错误，包含错误码、消息和提示
pub trait StreamErrorCallback: FnMut(String, AppError) + Send + 'static {}
impl<T> StreamErrorCallback for T where T: FnMut(String, AppError) + Send + 'static {}

/// 工具执行回调：当工具执行状态变化时调用
///
pub trait ToolExecuteCallback:
    FnMut(String, HashMap<String, MessageToolCall>) + Send + 'static
{
}
impl<T> ToolExecuteCallback for T where
    T: FnMut(String, HashMap<String, MessageToolCall>) + Send + 'static
{
}

/// 消息删除回调：当消息被删除时调用
///
/// 参数:
/// - `chat_id`: 聊天的唯一标识符
/// - `message_ids`: 被删除的消息ID列表
pub trait MessagesDeleteCallback: FnMut(String, Vec<String>) + Send + 'static {}
impl<T> MessagesDeleteCallback for T where T: FnMut(String, Vec<String>) + Send + 'static {}

/// 用户消息已保存回调：当用户消息保存到数据库后调用
///
/// 参数:
/// - `temp_message_id`: 前端临时消息ID（从 LlmMessage.id 中提取）
/// - `saved_message_id`: 数据库保存后的真实消息ID
pub trait UserMessageSavedCallback: FnMut(String, String) + Send + 'static {}
impl<T> UserMessageSavedCallback for T where T: FnMut(String, String) + Send + 'static {}

/// 消息服务
#[derive(Clone)]
pub struct MessageService {
    repository: MessageRepository,
    provider_service: Arc<ProviderService>,
    chat_service: Arc<SessionService>,
    mcp_service: Arc<McpService>,
    storage_service: Arc<StorageService>,
    /// Per-stream cancellation tokens keyed by `stream_id`. Inserted at the
    /// top of `call_llm_api_stream` once the stream id is generated; removed
    /// when the stream terminates (success, error, or external cancel) via
    /// the RAII `StreamCancellationGuard` below. The Arc<Mutex<...>> wrapper
    /// is necessary because `MessageService` is cloned across IPC handlers
    /// (`State::inner().clone()`), and every clone must observe the same
    /// registry so `cancel_stream` can find tokens registered by streaming
    /// tasks running on other clones.
    stream_cancellations: Arc<TokioMutex<HashMap<String, CancellationToken>>>,
}

/// RAII guard that removes a stream's cancellation token from
/// `MessageService::stream_cancellations` on drop. Spawns a detached cleanup
/// task because the registry uses a Tokio `Mutex` (async-only acquire). The
/// best-effort cleanup is acceptable: stream ids are UUIDs (never reused),
/// the token itself is dropped with the function frame regardless, and the
/// only downside of a stranded entry is a tiny amount of memory until the
/// process exits.
struct StreamCancellationGuard {
    cancellations: Arc<TokioMutex<HashMap<String, CancellationToken>>>,
    stream_id: String,
}

impl Drop for StreamCancellationGuard {
    fn drop(&mut self) {
        // No active Tokio runtime — best-effort cleanup degrades to a leak
        // rather than a secondary panic. Production paths always run under
        // Tauri's Tokio runtime; this branch only triggers in test/teardown
        // edge cases (e.g. a sync setup helper dropping the service).
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let cancellations = self.cancellations.clone();
        let stream_id = std::mem::take(&mut self.stream_id);
        tokio::spawn(async move {
            let mut guard = cancellations.lock().await;
            guard.remove(&stream_id);
        });
    }
}

impl MessageService {
    pub fn new(
        db: Arc<Database>,
        provider_service: Arc<ProviderService>,
        chat_service: Arc<SessionService>,
        mcp_service: Arc<McpService>,
        storage_service: Arc<StorageService>,
    ) -> Self {
        Self {
            repository: MessageRepository::new(db),
            provider_service,
            chat_service,
            mcp_service,
            storage_service,
            stream_cancellations: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    /// Cancel an in-flight streaming chat by its `stream_id`.
    ///
    /// Looks up the registered `CancellationToken` and fires `.cancel()`,
    /// which propagates through `ChatOptions::signal` →
    /// `SimpleStreamOptions::base.signal` → hand-ai's wrapper-level
    /// `select!` gate, aborting the provider stream within ~100ms (see
    /// hand-ai 0.2.0 `model-v0.2.0` tag).
    ///
    /// Returns silently if `stream_id` is unknown — racing a natural Done
    /// event against a Stop click is normal, and the user-visible behavior
    /// (stream ends within 500ms) is the same either way.
    pub async fn cancel_stream(&self, stream_id: &str) {
        let guard = self.stream_cancellations.lock().await;
        if let Some(token) = guard.get(stream_id) {
            token.cancel();
            tracing::info!(
                "[MessageService::cancel_stream] cancelled stream {}",
                stream_id
            );
        } else {
            tracing::debug!(
                "[MessageService::cancel_stream] stream {} not found (already finished?)",
                stream_id
            );
        }
    }

    /// Test-only: insert a `CancellationToken` into the per-stream registry
    /// so unit tests can exercise `cancel_stream` without spinning up an
    /// actual streaming chat. Exposed via `pub(crate)` under `#[cfg(test)]`
    /// to avoid widening production visibility of `stream_cancellations`.
    #[cfg(test)]
    pub(crate) async fn register_test_token(&self, stream_id: String, token: CancellationToken) {
        self.stream_cancellations
            .lock()
            .await
            .insert(stream_id, token);
    }

    /// 发送消息
    pub async fn send_user_message(
        &self,
        request: UserMessageSendRequest,
    ) -> Result<MessageResponse, AppError> {
        tracing::info!(
            "[MessageService::send_message] Starting to send message for chat_id: {}",
            request.chat_id
        );
        tracing::debug!(
            "[MessageService::send_message] Request details: {:?}",
            request
        );

        let UserMessageSendRequest {
            chat_id,
            content,
            temp_user_message_id,
            attachments,
        } = request;

        // 1. 获取聊天配置
        let chat = self.get_chat_config(&chat_id).await.map_err(|e| {
            let error = format!("Failed to get chat config: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?;
        let config = Self::message_config_from_chat(&chat);

        // 2. 获取 turn_id 并保存用户消息
        let turn_id_value = self
            .repository
            .get_next_turn_id(&chat_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to get next turn_id: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;
        let turn_id = Some(turn_id_value);

        let user_message_id = self
            .save_user_message(&chat_id, &content, config.clone(), turn_id, attachments)
            .await
            .map_err(|e| {
                let error = format!("Failed to save user message: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message] User message saved with ID: {}",
            user_message_id
        );

        if !temp_user_message_id.is_empty() {
            tracing::info!(
                "[MessageService::send_message] Temp message ID {} mapped to {}",
                temp_user_message_id,
                user_message_id
            );
        }

        // 3. 基于 turn 构建完整请求
        let final_request = self
            .build_message_request(&chat_id, turn_id_value)
            .await
            .map_err(|e| {
                let error = format!("Failed to build request from turn: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message] Built request with {} messages",
            final_request.messages.len()
        );

        // 4. 调用实际的 LLM API
        let (mut llm_response, generated_images) = self.call_llm_api(&final_request).await?;

        // 5. 保存助手消息到数据库
        let (assistant_message_id, processed_tool_calls, generated_assets) = self
            .save_assistant_message(
                &chat_id,
                llm_response.content.clone(),
                llm_response.reasoning.clone(),
                llm_response.tool_calls.clone(),
                Some(config.clone()),
                Utc::now().timestamp_millis(),
                llm_response.duration.unwrap_or(0),
                llm_response.input_tokens,
                llm_response.output_tokens,
                llm_response.total_tokens,
                turn_id,
                generated_images,
            )
            .await
            .map_err(|e| {
                let error = format!("Failed to save assistant message: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message] Assistant message saved with ID: {}",
            assistant_message_id
        );

        llm_response.chat_id = chat_id.clone();
        llm_response.message_id = assistant_message_id.clone();
        llm_response.tool_calls = processed_tool_calls.clone();
        llm_response.generated_assets = generated_assets.clone();

        let response = llm_response;

        tracing::info!(
            "[MessageService::send_message] Successfully completed for chat_id: {}, message_id: {}",
            chat_id,
            assistant_message_id
        );
        Ok(response)
    }

    /// 发送用户流式消息 - 处理完整的聊天逻辑包括消息保存
    pub async fn send_user_message_stream(
        &self,
        request: UserMessageSendRequest,
        start_callback: impl StreamStartCallback,
        streaming_callback: impl StreamingCallback,
        end_callback: impl StreamEndCallback,
        mut error_callback: impl StreamErrorCallback,
        mut user_message_saved_callback: impl UserMessageSavedCallback,
    ) {
        tracing::info!(
            "[MessageService::send_user_message_stream] Starting to send streaming message for chat_id: {:?}",
            request.chat_id
        );

        // 早期错误的临时 user_message_id
        let error_stream_id = &request.temp_user_message_id;

        // 验证消息角色
        // 2. 保存用户消息到数据库
        let chat_id = &request.chat_id;

        // 从 chats 表获取配置参数
        let chat = match self.get_chat_config(chat_id).await {
            Ok(chat) => chat,
            Err(e) => {
                let error = format!("Failed to get chat config: {}", e);
                tracing::error!(
                    "[MessageService::send_user_message_stream] Database error: {}",
                    error
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        let config = Self::message_config_from_chat(&chat);
        // 获取下一个 turn_id 用于这轮对话
        let turn_id = match self.repository.get_next_turn_id(chat_id).await {
            Ok(id) => Some(id),
            Err(e) => {
                let error = format!("Failed to get next turn_id: {}", e);
                tracing::error!(
                    "[MessageService::send_user_message_stream] Database error: {}",
                    error
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        let user_message_id = match self
            .save_user_message(
                chat_id,
                &request.content,
                config.clone(),
                turn_id.clone(),
                request.attachments.clone(),
            )
            .await
        {
            Ok(id) => id,
            Err(e) => {
                let error = format!("Failed to save user message: {}", e);
                tracing::error!(
                    "[MessageService::send_user_message_stream] Database error: {}",
                    error
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::send_user_message_stream] User message saved with ID: {}",
            user_message_id
        );

        // 如果前端在最后一条消息中提供了临时消息ID，通知前端替换为真实ID
        if !request.temp_user_message_id.is_empty() {
            user_message_saved_callback(
                request.temp_user_message_id.clone(),
                user_message_id.clone(),
            );
            tracing::info!(
                "[MessageService::send_user_message_stream] User message ID mapping: {} -> {}",
                request.temp_user_message_id,
                user_message_id
            );
        }

        // 3. 获取 turn_id 并使用 build_request_from_turn 构建完整请求（包括 system message 和历史消息）
        let turn_id_value = match turn_id {
            Some(id) => id,
            None => {
                let err = AppError::internal_error("Failed to get turn_id for new message");
                tracing::error!("[MessageService::send_user_message_stream] Missing turn_id");
                error_callback(error_stream_id.clone(), err);
                return;
            }
        };

        let final_request = match self.build_message_request(chat_id, turn_id_value).await {
            Ok(req) => req,
            Err(e) => {
                tracing::error!(
                    "[MessageService::send_user_message_stream] Failed to build request: {}",
                    e
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::send_user_message_stream] Built request with {} messages",
            final_request.messages.len()
        );

        // 4. 创建包装的 end_callback，在保存消息后调用原始回调
        let end_callback_wrapper = Self::wrap_end_callback_with_save(
            self.repository.clone(),
            chat_id.clone(),
            Some(config.clone()),
            turn_id,
            end_callback,
        );

        // 5. 调用流式 LLM API（使用构建好的完整请求）
        self.call_llm_api_stream(
            &final_request,
            start_callback,
            streaming_callback,
            end_callback_wrapper,
            error_callback,
        )
        .await;
    }

    /// 流式重发用户消息 - 删除该消息之后的所有消息，然后重新发送（流式）
    pub async fn resend_user_message_stream(
        &self,
        message_id: UUID,
        content: Option<String>,
        start_callback: impl StreamStartCallback,
        streaming_callback: impl StreamingCallback,
        end_callback: impl StreamEndCallback,
        mut error_callback: impl StreamErrorCallback,
        mut messages_delete_callback: impl MessagesDeleteCallback,
    ) {
        tracing::info!(
            "[MessageService::resend_message_stream] Resending user message (stream): {}",
            message_id
        );

        // 为早期错误生成一个临时 stream_id
        let error_stream_id = uuid::Uuid::new_v4().to_string();

        // 1. 获取要重发的消息
        let mut message = match self.get_message(message_id.clone()).await {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(
                    "[MessageService::resend_message_stream] Failed to get message {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        // 2. 验证消息是否为用户消息
        if message.role != LlmMessageRole::User {
            let err = AppError::validation_error("Can only resend user messages");
            tracing::error!(
                "[MessageService::resend_message_stream] Validation failed for message_id {}: not a user message",
                message_id
            );
            error_callback(error_stream_id, err);
            return;
        }

        // 3. 更新消息内容（如果提供了新的内容）
        if let Some(new_content) = content {
            let update_time = Utc::now().timestamp_millis();
            if let Err(e) = self
                .repository
                .update_message_content(&message_id, &new_content, update_time)
                .await
            {
                tracing::error!(
                    "[MessageService::resend_message_stream] Failed to update message content {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }

            message.content = new_content;
            message.updated_at = update_time;
        }

        // 4. 删除该消息之后的所有消息
        let deleted_message_ids = match self
            .repository
            .delete_messages_after(&message.session_id, &message_id)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!(
                    "[MessageService::resend_message_stream] Failed to delete messages after {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::resend_message_stream] Deleted {} messages after message_id {}",
            deleted_message_ids.len(),
            message_id
        );

        // 通知前端消息已被删除
        if !deleted_message_ids.is_empty() {
            messages_delete_callback(message.session_id.clone(), deleted_message_ids);
        }

        // 5. 获取聊天配置
        let chat = match self.chat_service.get_chat(message.session_id.clone()).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    "[MessageService::resend_message_stream] Failed to get chat config: {}",
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        // 6. 使用最新聊天配置更新用户消息的配置
        let updated_config = MessageConfig {
            model_id: chat.model_id.clone(),
            provider_id: chat.provider_id.clone(),
            temperature: chat.temperature,
            top_p: chat.top_p,
            top_k: chat.top_k,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
            turn_count: chat.turn_count,
            reasoning: chat.reasoning.clone(),
        };

        let update_time = Utc::now().timestamp_millis();
        if let Err(e) = self
            .repository
            .update_message_config(&message_id, Some(&updated_config), update_time)
            .await
        {
            tracing::error!(
                "[MessageService::resend_message_stream] Failed to update message config: {}",
                e
            );
            error_callback(error_stream_id, e);
            return;
        }

        message.config = Some(updated_config.clone());

        // 7. 获取 turn_id（使用原消息的 turn_id）
        let turn_id = match message.turn_id {
            Some(id) => id,
            None => {
                let err = AppError::validation_error(
                    "Cannot resend message without turn_id. This message may be from an older version.",
                );
                tracing::error!(
                    "[MessageService::resend_message_stream] Message {} has no turn_id",
                    message_id
                );
                error_callback(error_stream_id, err);
                return;
            }
        };

        // 8. 使用 build_request_from_turn 重新构建请求
        let resend_request = match self
            .build_message_request(&message.session_id, turn_id)
            .await
        {
            Ok(req) => req,
            Err(e) => {
                tracing::error!(
                    "[MessageService::resend_message_stream] Failed to build request: {}",
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::resend_message_stream] Sending stream request for chat {}",
            message.session_id
        );

        // 9. 包装 end_callback 以保存助手消息
        let end_callback_wrapper = Self::wrap_end_callback_with_save(
            self.repository.clone(),
            message.session_id.clone(),
            message.config.clone(),
            Some(turn_id),
            end_callback,
        );

        // 10. 直接调用 LLM API 流式方法
        self.call_llm_api_stream(
            &resend_request,
            start_callback,
            streaming_callback,
            end_callback_wrapper,
            error_callback,
        )
        .await;
    }

    /// 流式重新生成消息 - 删除当前消息，根据本轮(turn_id)消息重新构建请求并发送给 LLM
    pub async fn regenerate_assistant_message_stream(
        &self,
        message_id: UUID,
        start_callback: impl StreamStartCallback,
        streaming_callback: impl StreamingCallback,
        end_callback: impl StreamEndCallback,
        mut error_callback: impl StreamErrorCallback,
        mut messages_delete_callback: impl MessagesDeleteCallback,
    ) {
        tracing::info!(
            "[MessageService::regenerate_message_stream] Regenerating message (stream): {}",
            message_id
        );

        // 为早期错误生成一个临时 stream_id
        let error_stream_id = uuid::Uuid::new_v4().to_string();

        // 1. 获取要重新生成的消息
        let message = match self.get_message(message_id.clone()).await {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(
                    "[MessageService::regenerate_message_stream] Failed to get message {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        // 2. 验证消息是否为助手消息
        if message.role != LlmMessageRole::Assistant {
            let err = AppError::validation_error("Can only regenerate assistant messages");
            tracing::error!(
                "[MessageService::regenerate_message_stream] Validation failed for message_id {}: not an assistant message",
                message_id
            );
            error_callback(error_stream_id, err);
            return;
        }

        // 3. 删除当前消息及之后的所有消息
        let deleted_message_ids = match self
            .repository
            .delete_message_and_after(&message.session_id, &message_id)
            .await
        {
            Ok(ids) => {
                tracing::info!(
                    "[MessageService::regenerate_message_stream] Deleted {} messages (including current)",
                    ids.len()
                );
                ids
            }
            Err(e) => {
                tracing::error!(
                    "[MessageService::regenerate_message_stream] Failed to delete message and after {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        // 通知前端消息已被删除
        if !deleted_message_ids.is_empty() {
            messages_delete_callback(message.session_id.clone(), deleted_message_ids);
        }

        // 4. 验证消息必须有 turn_id
        let turn_id = match message.turn_id {
            Some(id) => id,
            None => {
                let err = AppError::validation_error(
                    "Cannot regenerate message without turn_id. This message may be from an older version.",
                );
                tracing::error!(
                    "[MessageService::regenerate_message_stream] Message {} has no turn_id",
                    message_id
                );
                error_callback(error_stream_id, err);
                return;
            }
        };

        // 5. 获取聊天配置并构建请求
        let regenerate_request = match self
            .build_message_request(&message.session_id, turn_id)
            .await
        {
            Ok(req) => req,
            Err(e) => {
                tracing::error!(
                    "[MessageService::regenerate_message_stream] Failed to build request: {}",
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::regenerate_message_stream] Sending stream request for chat {}",
            message.session_id
        );

        // 8. 包装 end_callback 以保存助手消息
        let end_callback_wrapper = Self::wrap_end_callback_with_save(
            self.repository.clone(),
            message.session_id.clone(),
            message.config.clone(),
            Some(turn_id),
            end_callback,
        );

        // 9. 直接调用 LLM API 流式方法
        self.call_llm_api_stream(
            &regenerate_request,
            start_callback,
            streaming_callback,
            end_callback_wrapper,
            error_callback,
        )
        .await;
    }

    /// 执行工具调用并发送结果给模型继续对话
    pub async fn execute_tool_calls(
        &self,
        message_id: String,
        tool_call_ids: Vec<String>,
        start_callback: impl StreamStartCallback,
        streaming_callback: impl StreamingCallback,
        end_callback: impl StreamEndCallback,
        mut error_callback: impl StreamErrorCallback,
        mut tool_execute_callback: impl ToolExecuteCallback,
        mut messages_delete_callback: impl MessagesDeleteCallback,
    ) {
        tracing::info!(
            "[MessageService::execute_tool_calls] Executing tool calls {:?} for message: {}",
            tool_call_ids,
            message_id
        );

        // 为早期错误生成一个临时 stream_id
        let error_stream_id = uuid::Uuid::new_v4().to_string();

        if tool_call_ids.is_empty() {
            let err = AppError::validation_error("At least one tool_call_id is required");
            error_callback(error_stream_id, err);
            return;
        }

        // 1. 获取消息
        tracing::debug!(
            "[MessageService::execute_tool_calls] Attempting to get message with ID: {}",
            message_id
        );
        let message = match self.get_message(message_id.clone()).await {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to get message {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        // 删除之前执行产生的后续消息（Tool 消息和 Assistant 响应），避免重复追加
        let deleted_message_ids = match self
            .repository
            .delete_messages_after(&message.session_id, &message_id)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to delete messages after {}: {}",
                    message_id,
                    e
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        // 通知前端消息已被删除
        if !deleted_message_ids.is_empty() {
            messages_delete_callback(message.session_id.clone(), deleted_message_ids);
        }

        // 2. 验证消息是否为 assistant消息且包含工具调用
        if message.role != LlmMessageRole::Assistant {
            let err =
                AppError::validation_error("Can only execute tool calls on assistant messages");
            error_callback(error_stream_id, err);
            return;
        }

        // 3. 从数据库中获取工具调用信息
        let stored_tool_calls = match message.tool_calls.as_ref() {
            Some(calls) => calls,
            None => {
                let err = AppError::validation_error("Message does not contain any tool calls");
                error_callback(error_stream_id, err);
                return;
            }
        };

        // 4. 根据 tool_call_ids 找到要执行的工具调用
        let mut selected_tool_calls = Vec::new();
        for tool_call_id in &tool_call_ids {
            match stored_tool_calls.iter().find(|tc| &tc.id == tool_call_id) {
                Some(call) => selected_tool_calls.push(call.clone()),
                None => {
                    let err = AppError::validation_error(&format!(
                        "Tool call with ID {} not found in message",
                        tool_call_id
                    ));
                    error_callback(error_stream_id.clone(), err);
                    return;
                }
            }
        }

        // 5. 更新工具调用状态为 Executing 并触发回调
        let executing_status_map: HashMap<String, MessageToolExecutionStatus> = selected_tool_calls
            .iter()
            .map(|tool_call| (tool_call.id.clone(), MessageToolExecutionStatus::Executing))
            .collect();

        let executing_updates = match self
            .update_tool_call_status(&message_id, &executing_status_map)
            .await
        {
            Ok(calls) => calls,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to update tool status to executing: {}",
                    e
                );
                Vec::new()
            }
        };

        if !executing_updates.is_empty() {
            let executing_map: HashMap<_, _> = executing_updates
                .into_iter()
                .map(|call| (call.id.clone(), call))
                .collect();
            tool_execute_callback(message_id.clone(), executing_map);
        }

        // 6. 执行所有工具调用并收集结果
        let mut tool_results = Vec::new();

        for tool_call in &selected_tool_calls {
            tracing::info!(
                "[MessageService::execute_tool_calls] Executing tool {} for message {}",
                tool_call.id,
                message.id
            );

            let result = match self
                .mcp_service
                .execute_tool(&tool_call.function.name, &tool_call.function.arguments)
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    tracing::error!(
                        "[MessageService::execute_tool_calls] Tool execution failed: {}",
                        error
                    );
                    format!("工具执行失败: {}", error.message)
                }
            };

            tool_results.push((tool_call.id.clone(), result));
        }

        tracing::info!(
            "[MessageService::execute_tool_calls] Collected {} tool results",
            tool_results.len()
        );

        // 7. 保存工具执行结果并触发回调
        let results_map: HashMap<String, String> = tool_results.iter().cloned().collect();

        let completed_updates = match self
            .update_tool_call_results(&message_id, &results_map)
            .await
        {
            Ok(calls) => calls,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to update tool results: {}",
                    e
                );
                Vec::new()
            }
        };

        if !completed_updates.is_empty() {
            let completed_map: HashMap<_, _> = completed_updates
                .into_iter()
                .map(|call| (call.id.clone(), call))
                .collect();
            tool_execute_callback(message_id.clone(), completed_map);
        }

        // 8. 检查同一消息中的所有工具调用是否都已完成
        // 重新获取消息以获取最新的工具调用状态
        let updated_message = match self.get_message(message_id.clone()).await {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to get updated message: {}",
                    e
                );
                error_callback(error_stream_id.clone(), e);
                return;
            }
        };

        // 检查是否还有未完成的工具调用（不论是自动还是手动执行模式）
        if let Some(tool_calls) = &updated_message.tool_calls {
            let has_pending_tools = tool_calls
                .iter()
                .any(|call| call.execution_status == MessageToolExecutionStatus::Pending);

            if has_pending_tools {
                tracing::info!(
                    "[MessageService::execute_tool_calls] Still has pending tools, waiting for execution"
                );
                // 还有未完成的工具，直接返回，不继续调用模型
                return;
            }
        }

        tracing::info!(
            "[MessageService::execute_tool_calls] All tools completed, continuing to call model"
        );

        // 9. 所有工具执行完成后，根据工具调用结果批量创建 Tool 消息
        // 使用 updated_message 中的 tool_calls（包含已存储的结果）
        if let Some(tool_calls) = &updated_message.tool_calls {
            let timestamp = Utc::now().timestamp_millis();
            let mut tool_result_messages = Vec::new();

            // 遍历所有已完成的工具调用，创建对应的 Tool 消息
            for tool_call in tool_calls {
                if tool_call.execution_status != MessageToolExecutionStatus::Pending {
                    if let Some(result) = &tool_call.result {
                        let tool_result_message = Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: message.session_id.clone(),
                            role: LlmMessageRole::Tool,
                            content: result.clone(),
                            reasoning: None,
                            tool_calls: None,
                            turn_id: message.turn_id.clone(),
                            tool_call_id: Some(tool_call.id.clone()),
                            config: None,
                            attachments: None,
                            generated_assets: None,
                            input_tokens: None,
                            output_tokens: None,
                            total_tokens: None,
                            start_time: None,
                            end_time: None,
                            duration: None,
                            created_at: timestamp,
                            updated_at: timestamp,
                        };
                        tool_result_messages.push(tool_result_message);
                    }
                }
            }

            // 批量保存工具结果消息
            if !tool_result_messages.is_empty() {
                if let Err(e) = self
                    .repository
                    .create_messages_batch(&tool_result_messages)
                    .await
                {
                    error_callback(error_stream_id.clone(), e);
                    return;
                }

                tracing::info!(
                    "[MessageService::execute_tool_calls] Saved {} tool result messages",
                    tool_result_messages.len()
                );
            }
        }

        // 10. 验证并获取 turn_id
        let turn_id = match message.turn_id {
            Some(id) => id,
            None => {
                let err = AppError::validation_error(
                    "Cannot execute tool calls for message without turn_id",
                );
                tracing::error!(
                    "[MessageService::execute_tool_calls] Message {} has no turn_id",
                    message.id
                );
                error_callback(error_stream_id, err);
                return;
            }
        };

        // 11. 构建包含工具调用结果的新请求
        let request = match self
            .build_message_request(&message.session_id, turn_id)
            .await
        {
            Ok(req) => req,
            Err(e) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to build request: {}",
                    e
                );
                error_callback(error_stream_id, e);
                return;
            }
        };

        tracing::info!(
            "[MessageService::execute_tool_calls] Request: {:?}",
            request
        );

        // 创建包装的 end_callback，在保存消息后调用原始回调
        let end_callback_wrapper = Self::wrap_end_callback_with_save(
            self.repository.clone(),
            message.session_id.clone(),
            message.config.clone(),
            message.turn_id.clone(),
            end_callback,
        );

        // 调用流式 LLM API 处理响应（不再返回值，通过回调处理）
        self.call_llm_api_stream(
            &request,
            start_callback,
            streaming_callback,
            end_callback_wrapper,
            error_callback,
        )
        .await;
    }

    /// 调用 LLM API
    async fn call_llm_api(
        &self,
        request: &MessageRequest,
    ) -> Result<(MessageResponse, Vec<LlmGeneratedImage>), AppError> {
        tracing::info!(
            "[MessageService::call_llm_api] Calling LLM API with provider: {}, model: {}",
            request.provider_id,
            request.model_id
        );

        // 1. 获取聊天配置
        let chat = if let Some(chat_id) = &request.chat_id {
            self.get_chat_config(chat_id).await.map_err(|e| {
                let error = format!("Failed to get chat config: {}", e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                e
            })?
        } else {
            return Err(AppError::validation_error("Chat ID is required"));
        };

        // 2. 获取供应商配置
        let provider = self
            .provider_service
            .get_provider(&request.provider_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to get provider {}: {}", request.provider_id, e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                AppError::validation_error(&error)
            })?;

        // 检查 API Key 是否存在
        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::call_llm_api] Provider {} has empty API key",
                request.provider_id
            );
            return Err(AppError::validation_error(
                "Provider has no API key configured",
            ));
        }

        // 3. 构造 chat_engine ChatProvider（不再创建 handbox-llm 客户端）
        let chat_provider = ChatProvider {
            provider_type: provider.provider_type.clone(),
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        };

        // 4. 翻译 MessageRequest → chat_engine 输入：ChatMessage 列表 + hydrated_attachments
        //    使用合成 id `req-msg-{idx}` 作为 ChatOptions::hydrated_attachments 的 key，
        //    并以同一 id 标记 ChatMessage::attachment_ids 的存在性。
        let mut chat_messages: Vec<ChatMessage> = Vec::with_capacity(request.messages.len());
        let mut hydrated_attachments: HashMap<String, Vec<HydratedAttachment>> = HashMap::new();
        for (idx, msg) in request.messages.iter().enumerate() {
            let msg_id = format!("req-msg-{}", idx);
            let attachment_ids: Vec<String> = match msg.attachments.as_ref() {
                Some(atts) if !atts.is_empty() => {
                    let payload: Vec<HydratedAttachment> = atts
                        .iter()
                        .map(|a| HydratedAttachment {
                            name: a.name.clone(),
                            mime_type: a.mime_type.clone(),
                            data: a.data.clone(),
                        })
                        .collect();
                    let ids: Vec<String> = (0..payload.len())
                        .map(|i| format!("{}-att-{}", msg_id, i))
                        .collect();
                    hydrated_attachments.insert(msg_id.clone(), payload);
                    ids
                }
                _ => Vec::new(),
            };
            chat_messages.push(ChatMessage {
                id: msg_id,
                role: msg.role.clone(),
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
                tool_calls: msg.tool_calls.as_ref().map(|v| {
                    v.iter()
                        .map(|tc| ChatToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        })
                        .collect()
                }),
                tool_call_id: msg.tool_call_id.clone(),
                attachment_ids,
            });
        }

        // 5. 构造 ChatOptions：复用 prepare_chat_tools / reasoning_effort 抽取逻辑。
        let chat_tools = self.prepare_chat_tools(&chat).await?;

        // reasoning_effort: 优先 chat.reasoning.reasoning_effort.effort；
        // 否则尝试 chat.reasoning.openrouter.effort（exclude=true 时跳过）。
        // 通过 supports_reasoning 门控，未声明 reasoning/thinking 参数的模型不发送。
        let supported_parameters = self.lookup_supported_parameters(&chat).await?;
        let supports_reasoning = Self::parameters_include(&supported_parameters, "reasoning")
            || Self::parameters_include(&supported_parameters, "thinking");
        let reasoning_effort: Option<String> = if supports_reasoning {
            chat.reasoning.as_ref().and_then(|cfg| {
                let from_effort = cfg
                    .reasoning_effort
                    .as_ref()
                    .and_then(|re| re.effort.as_ref())
                    .map(|e| match e {
                        LlmReasoningEffort::Minimal => "minimal".to_string(),
                        LlmReasoningEffort::Low => "low".to_string(),
                        LlmReasoningEffort::Medium => "medium".to_string(),
                        LlmReasoningEffort::High => "high".to_string(),
                    });
                if from_effort.is_some() {
                    return from_effort;
                }
                cfg.openrouter.as_ref().and_then(|or_cfg| {
                    if or_cfg.exclude == Some(true) {
                        return None;
                    }
                    or_cfg
                        .effort
                        .as_ref()
                        .and_then(|s| match s.to_ascii_lowercase().as_str() {
                            "minimal" | "low" | "medium" | "high" => Some(s.to_ascii_lowercase()),
                            _ => None,
                        })
                })
            })
        } else {
            None
        };

        // signal: 当前 services/message.rs 没有 CancellationToken 来源；M2-T2d 落地。
        let chat_options = ChatOptions {
            temperature: Self::normalize_numeric(chat.temperature),
            max_tokens: Self::normalize_numeric(chat.max_tokens)
                .and_then(|n| u32::try_from(n).ok()),
            tools: chat_tools,
            reasoning_effort,
            signal: None,
            hydrated_attachments,
        };

        // 6. 调用 chat_engine 非流式 API
        let start_time = std::time::Instant::now();
        let chunk = chat_engine::complete_chat(
            &chat_provider,
            &request.model_id,
            &chat_messages,
            chat_options,
        )
        .await
        .map_err(|err| {
            tracing::error!(
                "[MessageService::call_llm_api] chat_engine::complete_chat returned error: {}",
                err.message
            );
            err
        })?;

        let duration = start_time.elapsed().as_millis() as i64;

        // 7. 由 ChatChunk 直接构建 MessageResponse（取代旧 convert_from_api_response）。
        let tool_calls: Option<Vec<MessageToolCall>> = chunk.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|tc| MessageToolCall {
                    id: tc.id.clone(),
                    tool_type: "function".to_string(),
                    function: LlmToolFunction {
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    },
                    execution_mode: MessageToolExecutionMode::default(),
                    execution_status: MessageToolExecutionStatus::default(),
                    result: None,
                })
                .collect()
        });

        let response = MessageResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: uuid::Uuid::new_v4().to_string(), // 临时ID，调用方覆盖
            content: chunk.content.unwrap_or_default(),
            reasoning: chunk.reasoning,
            tool_calls,
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: chunk.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: chunk.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: chunk.usage.as_ref().map(|u| u.total_tokens),
            duration: Some(duration),
            generated_assets: None,
        };

        // TODO(post-M3): hand-ai 0.2.0's AssistantContentBlock has no Image variant.
        // Image-generating models (DALL-E, gpt-image-1) stop returning generated
        // images via this path. Re-enable when hand-ai surfaces them.
        let generated_images: Vec<LlmGeneratedImage> = Vec::new();

        tracing::info!(
            "[MessageService::call_llm_api] chat_engine call successful, duration: {}ms",
            duration
        );

        Ok((response, generated_images))
    }

    /// 流式调用 LLM API
    pub async fn call_llm_api_stream(
        &self,
        request: &MessageRequest,
        mut start_callback: impl StreamStartCallback,
        mut streaming_callback: impl StreamingCallback,
        mut end_callback: impl StreamEndCallback,
        mut error_callback: impl StreamErrorCallback,
    ) {
        tracing::info!("[MessageService::call_llm_api_stream] Starting stream call with provider: {}, model: {}",
            request.provider_id, request.model_id);

        // 生成 streamId 和 messageId
        let stream_id = uuid::Uuid::new_v4().to_string();
        let message_id = uuid::Uuid::new_v4().to_string();

        // 注册 per-stream CancellationToken。`_cancel_guard` 在函数返回时（无论
        // 走哪条 early-return 路径，包括 panic 展开）触发 Drop，将本 stream_id
        // 从 registry 中移除，避免每条 termination 分支都重复写 remove() 调用。
        let cancel_token = CancellationToken::new();
        {
            let mut guard = self.stream_cancellations.lock().await;
            guard.insert(stream_id.clone(), cancel_token.clone());
        }
        let _cancel_guard = StreamCancellationGuard {
            cancellations: self.stream_cancellations.clone(),
            stream_id: stream_id.clone(),
        };

        // 1. 获取聊天配置
        let chat = if let Some(chat_id) = &request.chat_id {
            match self.get_chat_config(chat_id).await {
                Ok(chat) => chat,
                Err(e) => {
                    let error = format!("Failed to get chat config: {}", e);
                    tracing::error!("[MessageService::call_llm_api_stream] {}", error);
                    error_callback(stream_id.clone(), e);
                    return;
                }
            }
        } else {
            let err = AppError::validation_error("Chat ID is required for streaming");
            error_callback(stream_id.clone(), err);
            return;
        };

        // 2. 获取供应商配置
        let provider = match self
            .provider_service
            .get_provider(&request.provider_id)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                let error = format!("Failed to get provider {}: {}", request.provider_id, e);
                tracing::error!("[MessageService::call_llm_api_stream] {}", error);
                let err = AppError::validation_error(&error);
                error_callback(stream_id.clone(), err);
                return;
            }
        };

        // 验证API Key
        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::call_llm_api_stream] Provider {} has empty API key",
                request.provider_id
            );
            let err = AppError::validation_error("Provider has no API key configured");
            error_callback(stream_id.clone(), err);
            return;
        }

        let api_key_preview = if provider.api_key.len() > 8 {
            format!(
                "{}...{}",
                &provider.api_key[..4],
                &provider.api_key[provider.api_key.len() - 4..]
            )
        } else {
            "***".to_string()
        };
        tracing::info!(
            "[MessageService::call_llm_api_stream] Using provider: {} ({}), API key: {}",
            provider.name,
            provider.provider_type,
            api_key_preview
        );

        // 3. 构造 chat_engine ChatProvider（不再创建 handbox-llm 客户端）
        let chat_provider = ChatProvider {
            provider_type: provider.provider_type.clone(),
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        };

        // 4. 翻译 MessageRequest → chat_engine 输入：ChatMessage 列表 + hydrated_attachments
        //    使用合成 id `req-msg-{idx}` 作为 ChatOptions::hydrated_attachments 的 key，
        //    并以同一 id 标记 ChatMessage::attachment_ids 的存在性。
        let mut chat_messages: Vec<ChatMessage> = Vec::with_capacity(request.messages.len());
        let mut hydrated_attachments: HashMap<String, Vec<HydratedAttachment>> = HashMap::new();
        for (idx, msg) in request.messages.iter().enumerate() {
            let msg_id = format!("req-msg-{}", idx);
            let attachment_ids: Vec<String> = match msg.attachments.as_ref() {
                Some(atts) if !atts.is_empty() => {
                    let payload: Vec<HydratedAttachment> = atts
                        .iter()
                        .map(|a| HydratedAttachment {
                            name: a.name.clone(),
                            mime_type: a.mime_type.clone(),
                            data: a.data.clone(),
                        })
                        .collect();
                    let ids: Vec<String> = (0..payload.len())
                        .map(|i| format!("{}-att-{}", msg_id, i))
                        .collect();
                    hydrated_attachments.insert(msg_id.clone(), payload);
                    ids
                }
                _ => Vec::new(),
            };
            chat_messages.push(ChatMessage {
                id: msg_id,
                role: msg.role.clone(),
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
                tool_calls: msg.tool_calls.as_ref().map(|v| {
                    v.iter()
                        .map(|tc| ChatToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        })
                        .collect()
                }),
                tool_call_id: msg.tool_call_id.clone(),
                attachment_ids,
            });
        }

        // 5. 构造 ChatOptions：复用 prepare_chat_tools / reasoning_effort 抽取逻辑。
        let chat_tools: Vec<ChatTool> = match self.prepare_chat_tools(&chat).await {
            Ok(tools) => tools,
            Err(e) => {
                error_callback(stream_id.clone(), e);
                return;
            }
        };

        // reasoning_effort: 优先 chat.reasoning.reasoning_effort.effort；
        // 否则尝试 chat.reasoning.openrouter.effort（exclude=true 时跳过）。
        // 通过 supports_reasoning 门控，未声明 reasoning/thinking 参数的模型不发送。
        let supported_parameters = match self.lookup_supported_parameters(&chat).await {
            Ok(params) => params,
            Err(e) => {
                error_callback(stream_id.clone(), e);
                return;
            }
        };
        let supports_reasoning = Self::parameters_include(&supported_parameters, "reasoning")
            || Self::parameters_include(&supported_parameters, "thinking");
        let reasoning_effort: Option<String> = if supports_reasoning {
            chat.reasoning.as_ref().and_then(|cfg| {
                let from_effort = cfg
                    .reasoning_effort
                    .as_ref()
                    .and_then(|re| re.effort.as_ref())
                    .map(|e| match e {
                        LlmReasoningEffort::Minimal => "minimal".to_string(),
                        LlmReasoningEffort::Low => "low".to_string(),
                        LlmReasoningEffort::Medium => "medium".to_string(),
                        LlmReasoningEffort::High => "high".to_string(),
                    });
                if from_effort.is_some() {
                    return from_effort;
                }
                cfg.openrouter.as_ref().and_then(|or_cfg| {
                    if or_cfg.exclude == Some(true) {
                        return None;
                    }
                    or_cfg
                        .effort
                        .as_ref()
                        .and_then(|s| match s.to_ascii_lowercase().as_str() {
                            "minimal" | "low" | "medium" | "high" => Some(s.to_ascii_lowercase()),
                            _ => None,
                        })
                })
            })
        } else {
            None
        };

        // signal: 注入函数顶部注册的 CancellationToken；通过 chat_engine 透传到
        // hand-ai 的 SimpleStreamOptions::base.signal，由 hand-ai 0.2.0 的
        // wrapper-level select! gate 保证 ~100ms 内中止 provider 流。
        let chat_options = ChatOptions {
            temperature: Self::normalize_numeric(chat.temperature),
            max_tokens: Self::normalize_numeric(chat.max_tokens)
                .and_then(|n| u32::try_from(n).ok()),
            tools: chat_tools,
            reasoning_effort,
            signal: Some(cancel_token.clone()),
            hydrated_attachments,
        };

        // 6. 调用开始回调
        start_callback(stream_id.clone(), message_id.clone());

        // 7. 调用 chat_engine 流式 API
        let start_time = std::time::Instant::now();

        tracing::info!(
            "[MessageService::call_llm_api_stream] Calling chat_engine::stream_chat (provider={}, model={})",
            provider.provider_type,
            request.model_id
        );
        let mut stream = match chat_engine::stream_chat(
            &chat_provider,
            &request.model_id,
            &chat_messages,
            chat_options,
        )
        .await
        {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(
                    "[MessageService::call_llm_api_stream] chat_engine::stream_chat returned error: {}",
                    err.message
                );
                error_callback(stream_id.clone(), err);
                return;
            }
        };

        let mut accumulated_content = String::new();
        let mut accumulated_reasoning = String::new();
        // TODO(post-M3): hand-ai 0.2.0's AssistantContentBlock has no Image variant.
        // Image-generating models (DALL-E, gpt-image-1) stop returning generated
        // images via this path. Re-enable when hand-ai surfaces them.
        let accumulated_generated_images: Vec<LlmGeneratedImage> = Vec::new();
        let mut terminal_tool_calls: Vec<crate::models::llm_types::LlmToolCall> = Vec::new();
        let mut terminal_usage: Option<chat_engine::ChatUsage> = None;
        let mut chunk_count = 0;

        // 8. 处理 chat_engine 流式响应
        use futures::StreamExt;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    // 终止 chunk 上 chat_engine 会附带 usage；非终止 chunk 通常为 None。
                    if let Some(usage) = chunk.usage.clone() {
                        terminal_usage = Some(usage);
                    }

                    if let Some(text) = chunk.content.as_deref() {
                        if !text.is_empty() {
                            accumulated_content.push_str(text);
                        }
                    }

                    if let Some(reasoning_chunk) = chunk.reasoning.as_deref() {
                        if !reasoning_chunk.is_empty() {
                            accumulated_reasoning.push_str(reasoning_chunk);
                            tracing::info!(
                                "[MessageService::call_llm_api_stream] Received reasoning chunk: '{}', total accumulated: {} chars",
                                reasoning_chunk,
                                accumulated_reasoning.len()
                            );
                        }
                    }

                    // tool_calls 只在终止 chunk 上出现（chat_engine 保证）。
                    if let Some(tcs) = chunk.tool_calls.as_ref() {
                        terminal_tool_calls = tcs
                            .iter()
                            .map(|tc| crate::models::llm_types::LlmToolCall {
                                id: tc.id.clone(),
                                tool_type: "function".to_string(),
                                function: LlmToolFunction {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                },
                            })
                            .collect();
                    }

                    // 推送增量给前端（与原行为一致：每个 chunk 触发一次回调）。
                    let message_tool_calls = if terminal_tool_calls.is_empty() {
                        None
                    } else {
                        Some(
                            terminal_tool_calls
                                .iter()
                                .map(|tc| MessageToolCall::from(tc.clone()))
                                .collect(),
                        )
                    };

                    streaming_callback(StreamChunk {
                        stream_id: stream_id.clone(),
                        content: accumulated_content.clone(),
                        reasoning: chunk.reasoning.clone(),
                        tool_calls: message_tool_calls,
                        is_generating_assets: if accumulated_generated_images.is_empty() {
                            None
                        } else {
                            Some(true)
                        },
                    });
                    chunk_count += 1;

                    tracing::debug!(
                        "[MessageService::call_llm_api_stream] chat_engine chunk {}: content='{}', reasoning='{}'",
                        chunk_count,
                        chunk.content.as_deref().unwrap_or(""),
                        chunk.reasoning.as_deref().unwrap_or("")
                    );

                    // 添加小延迟以控制流速（保持与原行为一致，避免前端 UI 抖动）。
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                    if let Some(reason) = chunk.finish_reason.as_deref() {
                        tracing::info!(
                            "[MessageService::call_llm_api_stream] Stream finished with reason: {}",
                            reason
                        );
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "[MessageService::call_llm_api_stream] Stream error: {}",
                        e.message
                    );
                    error_callback(stream_id.clone(), e);
                    return;
                }
            }
        }

        let duration = start_time.elapsed().as_millis() as i64;
        let final_content = accumulated_content.clone();
        let images_to_persist = accumulated_generated_images;
        let generated_image_count = images_to_persist.len();
        let mut persisted_assets: Option<Vec<MessageAttachment>> = None;

        if generated_image_count > 0 {
            if let Some(chat_id) = request.chat_id.as_ref() {
                match self.persist_generated_assets(chat_id, &message_id, images_to_persist) {
                    Ok(assets) => {
                        persisted_assets = assets;
                    }
                    Err(err) => {
                        error_callback(stream_id.clone(), err);
                        return;
                    }
                }
            } else {
                tracing::warn!(
                    "[MessageService::call_llm_api_stream] Missing chat_id for generated images; skipping persistence"
                );
            }
        }

        tracing::info!(
            "[MessageService::call_llm_api_stream] Real streaming API call completed, chunks: {}, total_content_length: {}, duration: {}ms",
            chunk_count, accumulated_content.len(), duration
        );

        // 9. 构造消息配置并处理工具调用的执行模式
        let config = Self::message_config_from_chat(&chat);
        // 过滤掉空的工具调用（id 和 name 都为空的）
        let valid_tool_calls: Vec<_> = terminal_tool_calls
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.function.name.is_empty())
            .collect();

        let processed_tool_calls = if valid_tool_calls.is_empty() {
            None
        } else {
            // 先将 LlmToolCall 转换为 MessageToolCall
            let message_tool_calls: Vec<MessageToolCall> = valid_tool_calls
                .into_iter()
                .map(|tc| MessageToolCall::from(tc))
                .collect();
            // 然后根据配置更新执行模式
            Self::prepare_tool_calls(Some(message_tool_calls), &Some(config))
        };

        // 8. 构造 MessageResponse
        tracing::info!(
            "[MessageService::call_llm_api_stream] Creating final response with {} persisted images",
            generated_image_count
        );

        let response = MessageResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: message_id.clone(),
            content: final_content,
            reasoning: if accumulated_reasoning.is_empty() {
                None
            } else {
                Some(accumulated_reasoning.clone())
            },
            tool_calls: processed_tool_calls,
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: terminal_usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: terminal_usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: terminal_usage.as_ref().map(|u| u.total_tokens),
            duration: Some(duration),
            generated_assets: persisted_assets,
        };

        tracing::info!(
            "[MessageService::call_llm_api_stream] Stream completed - content: {} chars, reasoning: {} chars",
            accumulated_content.len(),
            accumulated_reasoning.len()
        );

        // 调用结束回调，传递 stream_id
        end_callback(stream_id, response);
    }

    /// 收集激活的 MCP 工具并翻译为 `ChatTool`（chat_engine 输入类型）。
    ///
    /// 同一个 helper 供流式与非流式两条 dispatch 路径共用——M2-T2c 之前流式
    /// 路径会再做一次 `LlmRequestTool` → `ChatTool` 的中转，现在直接生成
    /// `Vec<ChatTool>` 减少一次翻译。
    async fn prepare_chat_tools(&self, chat: &Session) -> Result<Vec<ChatTool>, AppError> {
        if chat.mcp_servers.is_empty() {
            return Ok(Vec::new());
        }

        // Extract server IDs from McpServerConfig
        let server_ids: Vec<String> = chat
            .mcp_servers
            .iter()
            .map(|config| config.server_id.clone())
            .collect();

        let servers = self.mcp_service.get_servers_by_ids(&server_ids).await?;

        let active_servers: Vec<McpServer> = servers
            .into_iter()
            .filter(|server| server.enabled && matches!(server.status, McpServerStatus::Ready))
            .collect();

        if active_servers.is_empty() {
            return Ok(Vec::new());
        }

        let mut tools: Vec<ChatTool> = Vec::new();

        for server in active_servers {
            // Find the server config to get enabled tools
            let server_config = chat
                .mcp_servers
                .iter()
                .find(|config| config.server_id == server.id);

            for tool in &server.tools {
                // Check if this tool is enabled for this server in the chat config
                let is_tool_enabled = server_config
                    .map(|config| {
                        // If enabledTools is empty, use all tools enabled in server settings
                        if config.enabled_tools.is_empty() {
                            server.enabled_tools.contains(&tool.name)
                        } else {
                            // Otherwise use the chat's enabled tools list
                            config.enabled_tools.contains(&tool.name)
                        }
                    })
                    .unwrap_or(false);

                if !is_tool_enabled {
                    continue;
                }

                let description = tool
                    .description
                    .clone()
                    .filter(|desc| !desc.is_empty())
                    .unwrap_or_else(|| {
                        let display_name = server
                            .display_name
                            .as_deref()
                            .filter(|name| !name.is_empty())
                            .unwrap_or(&server.name);
                        format!("MCP 服务器 {} 的工具 {}", display_name, tool.name)
                    });

                tools.push(ChatTool {
                    name: tool.name.clone(),
                    description,
                    parameters: tool.input_schema.clone(),
                });
            }
        }

        Ok(tools)
    }

    /// 过滤掉无效的数值参数（0 或负数）。
    ///
    /// 两条 dispatch 路径都会用它构造 `ChatOptions::{temperature, max_tokens}`，
    /// 因此放在 impl 层以单一定义服务两端（M2-T2c 之前两端各有一份本地定义）。
    fn normalize_numeric<T>(value: Option<T>) -> Option<T>
    where
        T: PartialOrd + Default,
    {
        value.filter(|v| *v > T::default())
    }

    async fn lookup_supported_parameters(
        &self,
        chat: &Session,
    ) -> Result<Option<Vec<String>>, AppError> {
        if let (Some(model_id), Some(provider_id)) = (&chat.model_id, &chat.provider_id) {
            if let Some(model) = self
                .provider_service
                .get_model(provider_id, model_id)
                .await?
            {
                return Ok(model.supported_parameters.clone());
            }
        }

        Ok(None)
    }

    fn parameters_include(list: &Option<Vec<String>>, key: &str) -> bool {
        list.as_ref()
            .map(|params| params.iter().any(|param| param == key))
            .unwrap_or(false)
    }

    /// 构造并保存用户消息
    async fn save_user_message(
        &self,
        chat_id: &str,
        content: &str,
        config: MessageConfig,
        turn_id: Option<i32>,
        attachments: Option<Vec<MessageRequestAttachment>>,
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        let stored_attachments =
            self.persist_request_attachments(chat_id, &message_id, attachments)?;

        let message = Message {
            id: message_id.clone(),
            session_id: chat_id.to_string(),
            role: LlmMessageRole::User,
            content: content.to_string(),
            reasoning: None, // 用户消息没有推理过程
            config: Some(config),
            tool_calls: None,
            turn_id,
            tool_call_id: None, // 用户消息没有关联的工具调用
            attachments: stored_attachments,
            generated_assets: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: Some(now),
            end_time: Some(now),
            duration: Some(0), // 用户消息无耗时
            created_at: now,
            updated_at: now,
        };

        self.repository.create_message(&message).await?;
        tracing::info!("[MessageService] User message saved: {}", message_id);
        Ok(message_id)
    }

    fn persist_request_attachments(
        &self,
        chat_id: &str,
        message_id: &str,
        attachments: Option<Vec<MessageRequestAttachment>>,
    ) -> Result<Option<Vec<MessageAttachment>>, AppError> {
        let Some(list) = attachments else {
            return Ok(None);
        };

        if list.is_empty() {
            return Ok(None);
        }

        let attachment_dir = self
            .storage_service
            .prepare_message_attachment_dir(chat_id, message_id)?;

        let mut stored = Vec::new();
        for (index, attachment) in list.into_iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            let extension = Self::extension_from_mime(&attachment.mime_type);
            let suggested_name = if attachment.name.trim().is_empty() {
                format!("attachment-{:02}.{}", index + 1, extension)
            } else {
                let mut name = attachment.name.clone();
                if !name.contains('.') {
                    name.push('.');
                    name.push_str(extension);
                }
                name
            };
            let file_path = attachment_dir.join(&suggested_name);
            fs::write(&file_path, &attachment.data).map_err(|e| {
                AppError::internal_error(&format!(
                    "Failed to write attachment to disk ({}): {}",
                    file_path.display(),
                    e
                ))
            })?;

            stored.push(MessageAttachment {
                id,
                name: suggested_name,
                mime_type: attachment.mime_type,
                size: attachment.data.len() as i64,
                path: file_path.to_string_lossy().into_owned(),
            });
        }

        Ok(Some(stored))
    }

    /// 处理工具调用的执行模式和状态
    /// 根据消息配置中的 MCP 服务器设置，为每个工具调用设置执行模式和初始状态
    ///
    /// 输入：已经转换为 MessageToolCall 的工具调用（可能使用了默认的执行模式）
    /// 输出：根据配置更新了执行模式的工具调用
    fn prepare_tool_calls(
        tool_calls: Option<Vec<MessageToolCall>>,
        config: &Option<MessageConfig>,
    ) -> Option<Vec<MessageToolCall>> {
        tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|mut call| {
                    // 默认为自动执行模式（如果还没有设置）
                    let mut execution_mode = call.execution_mode.clone();

                    // 根据配置查找工具所属的 MCP 服务器及其执行模式
                    if let Some(cfg) = config {
                        if let Some(mcp_servers) = &cfg.mcp_servers {
                            let tool_name = &call.function.name;

                            // 查找包含此工具的 MCP 服务器
                            for server in mcp_servers {
                                if server.enabled_tools.contains(tool_name) {
                                    // 根据服务器的执行模式设置工具的执行模式
                                    execution_mode = if server.execution_mode == "manual" {
                                        MessageToolExecutionMode::Manual
                                    } else {
                                        MessageToolExecutionMode::Auto
                                    };
                                    break;
                                }
                            }
                        }
                    }
                    // 更新执行模式
                    call.execution_mode = execution_mode;
                    call
                })
                .collect()
        })
    }

    /// 构造并保存AI响应消息
    /// 返回 (message_id, processed_tool_calls)
    ///
    /// 注意：tool_calls 应该已经是处理过的 MessageToolCall（包含执行模式和状态）
    async fn save_assistant_message(
        &self,
        chat_id: &str,
        content: String,
        reasoning: Option<String>,
        tool_calls: Option<Vec<MessageToolCall>>,
        config: Option<MessageConfig>,
        start_time_millis: i64,
        duration_ms: i64,
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
        total_tokens: Option<i32>,
        turn_id: Option<i32>,
        generated_images: Vec<LlmGeneratedImage>,
    ) -> Result<
        (
            String,
            Option<Vec<MessageToolCall>>,
            Option<Vec<MessageAttachment>>,
        ),
        AppError,
    > {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        // 处理工具调用的执行模式和状态
        let processed_tool_calls = Self::prepare_tool_calls(tool_calls, &config);
        let generated_assets =
            self.persist_generated_assets(chat_id, &message_id, generated_images)?;

        let message = Message {
            id: message_id.clone(),
            session_id: chat_id.to_string(),
            role: LlmMessageRole::Assistant,
            content: content.clone(),
            reasoning,
            tool_calls: processed_tool_calls.clone(),
            config,
            turn_id,
            tool_call_id: None,
            attachments: None,
            generated_assets: generated_assets.clone(),
            input_tokens,
            output_tokens,
            total_tokens,
            start_time: Some(start_time_millis),
            end_time: Some(now),
            duration: Some(duration_ms),
            created_at: now,
            updated_at: now,
        };

        self.repository.create_message(&message).await?;
        tracing::info!("[MessageService] Assistant message saved: {}", message_id);
        Ok((message_id, processed_tool_calls, generated_assets))
    }

    /// 获取聊天配置
    async fn get_chat_config(&self, chat_id: &str) -> Result<Session, AppError> {
        self.chat_service.get_chat(chat_id.to_string()).await
    }

    fn persist_generated_assets(
        &self,
        chat_id: &str,
        message_id: &str,
        generated_images: Vec<LlmGeneratedImage>,
    ) -> Result<Option<Vec<MessageAttachment>>, AppError> {
        if generated_images.is_empty() {
            return Ok(None);
        }

        let media_dir = self
            .storage_service
            .prepare_message_media_dir(chat_id, message_id)?;
        let mut stored_assets = Vec::new();

        for (index, image) in generated_images.into_iter().enumerate() {
            let data = BASE64_STANDARD.decode(image.data.as_bytes()).map_err(|e| {
                AppError::internal_error(&format!("Failed to decode generated image: {e}"))
            })?;
            let extension = Self::extension_from_mime(&image.mime_type);
            let file_name = format!("image-{:02}.{}", index + 1, extension);
            let file_path = media_dir.join(&file_name);
            let size = data.len() as i64;
            fs::write(&file_path, &data).map_err(|e| {
                AppError::internal_error(&format!(
                    "Failed to write generated image to disk ({}): {}",
                    file_path.display(),
                    e
                ))
            })?;

            stored_assets.push(MessageAttachment {
                id: uuid::Uuid::new_v4().to_string(),
                name: file_name,
                mime_type: image.mime_type.clone(),
                size,
                path: file_path.to_string_lossy().into_owned(),
            });
        }

        Ok(Some(stored_assets))
    }

    fn load_llm_attachments(
        attachments: &[MessageAttachment],
    ) -> Result<Vec<crate::models::llm_types::LlmMessageAttachment>, AppError> {
        let mut resources = Vec::new();
        for attachment in attachments {
            let data = fs::read(&attachment.path).map_err(|e| {
                AppError::internal_error(&format!(
                    "Failed to read attachment {} from {}: {}",
                    attachment.name, attachment.path, e
                ))
            })?;
            resources.push(crate::models::llm_types::LlmMessageAttachment {
                name: attachment.name.clone(),
                mime_type: attachment.mime_type.clone(),
                data,
            });
        }
        Ok(resources)
    }

    fn extension_from_mime(mime_type: &str) -> &'static str {
        match mime_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/png" => "png",
            other if other.ends_with("jpeg") => "jpg",
            other if other.ends_with("png") => "png",
            _ => "png",
        }
    }

    /// 构建请求开头的 System 段消息。
    ///
    /// 纯函数，便于单元测试。返回值依次包含：
    /// 1. 用户配置的 `system_prompt`（存在且去空白后非空时）；
    /// 2. 当 `generative_ui == Some(true)` 时，一条独立的、内容为
    ///    [`GENERATIVE_UI_PROMPT`] 的 System 消息。
    ///
    /// 两者共存——目录提示是追加的独立消息，绝不覆盖或截断用户的系统提示
    /// （VAL-INJECT-006）。注入仅在 `Some(true)` 时发生：`None` / `Some(false)`
    /// 均不注入（VAL-INJECT-002）。本函数仅产出请求消息，从不写回
    /// `session.system_prompt`（VAL-INJECT-003）；每次调用都是全新构建，故重复调用
    /// 不会叠加目录提示（VAL-INJECT-003 的幂等性）。
    ///
    /// 当 `generative_ui == Some(true)` 且 `genui_example` 为非空白文本时，再追加
    /// 第三条独立 System 段：把该范例（会话创建时由关联 GenUI 模板快照而来）框定为
    /// 应模仿的输出范例。范例只在目录提示之后注入；`generative_ui` 未开启时即便有
    /// 范例也不注入。
    fn build_system_messages(
        system_prompt: &Option<String>,
        generative_ui: Option<bool>,
        genui_example: Option<&str>,
    ) -> Vec<LlmMessage> {
        let mut messages = Vec::new();

        if let Some(prompt) = system_prompt {
            if !prompt.trim().is_empty() {
                messages.push(LlmMessage {
                    role: LlmMessageRole::System,
                    content: prompt.clone(),
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                    attachments: None,
                });
            }
        }

        if generative_ui == Some(true) {
            messages.push(LlmMessage {
                role: LlmMessageRole::System,
                content: GENERATIVE_UI_PROMPT.to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
                attachments: None,
            });

            // 关联了范例模板时，追加一条「模仿此范例」的 System 段（目录提示之后）。
            if let Some(example) = genui_example {
                let trimmed = example.trim();
                if !trimmed.is_empty() {
                    messages.push(LlmMessage {
                        role: LlmMessageRole::System,
                        content: format!("{GENERATIVE_UI_EXAMPLE_PREAMBLE}\n\n{trimmed}"),
                        reasoning: None,
                        tool_calls: None,
                        tool_call_id: None,
                        attachments: None,
                    });
                }
            }
        }

        messages
    }

    /// 根据 chat_id 和 turn_id 构建 MessageRequest
    ///
    /// 用于重新生成消息或执行工具调用时，根据特定轮次的消息历史构建请求
    ///
    /// # 参数
    /// - `chat_id`: 聊天 ID
    /// - `turn_id`: 轮次 ID（最大轮次 ID）
    async fn build_message_request(
        &self,
        chat_id: &str,
        turn_id: i32,
    ) -> Result<MessageRequest, AppError> {
        // 1. 获取聊天配置
        let chat = self.get_chat_config(chat_id).await?;

        // 2. 基于聊天配置构建消息配置
        let message_config = MessageConfig {
            model_id: chat.model_id.clone(),
            provider_id: chat.provider_id.clone(),
            temperature: chat.temperature,
            top_p: chat.top_p,
            top_k: chat.top_k,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
            turn_count: chat.turn_count,
            reasoning: chat.reasoning.clone(),
        };

        // 3. 构建消息数组
        let mut request_messages = Vec::new();

        // 添加系统提示词（用户配置的系统提示 + 可选的 generative-UI 目录提示）。
        // 通过 `build_system_messages` 集中生成，是所有发送路径共用的唯一注入点。
        request_messages.extend(Self::build_system_messages(
            &chat.system_prompt,
            chat.generative_ui,
            chat.genui_spec.as_deref(),
        ));

        // 4. 获取 turn_count，优先使用聊天配置中的值
        let turn_count = message_config.turn_count.unwrap_or(5); // 默认 5 轮

        // 5. 获取该聊天所有存在的 turn_id（去重排序）
        let all_turn_ids = self
            .repository
            .get_turn_ids_by_chat(&chat_id.to_string())
            .await?;

        // 6. 找到 turn_id 在数组中的位置，计算要获取的 turn_id 范围
        let (min_turn_id, max_turn_id) =
            if let Some(pos) = all_turn_ids.iter().position(|&id| id == turn_id) {
                // 计算起始位置：向前取 turn_count - 1 个轮次
                let start_pos = pos.saturating_sub((turn_count - 1) as usize);
                let min_id = all_turn_ids[start_pos];
                let max_id = turn_id;
                (min_id, max_id)
            } else {
                // 如果 turn_id 不在列表中（可能被删除），报错
                return Err(AppError::not_found(&format!(
                    "Turn {} not found in chat {}. Available turns: {:?}",
                    turn_id, chat_id, all_turn_ids
                )));
            };

        // 7. 根据计算出的 min 和 max turn_id 获取消息
        let turn_messages = self
            .repository
            .get_messages_by_turn_id_range(&chat_id.to_string(), min_turn_id, max_turn_id)
            .await?;

        for m in turn_messages.iter() {
            let mut attachments = Vec::new();

            // tracing::info!(
            //     "[build_message_request] Processing message: role={:?}, has_attachments={}, has_generated_assets={}, content_preview={}",
            //     m.role,
            //     m.attachments.as_ref().map(|a| a.len()).unwrap_or(0),
            //     m.generated_assets.as_ref().map(|g| g.len()).unwrap_or(0),
            //     if m.content.len() > 100 { &m.content[..100] } else { &m.content }
            // );

            // 恢复原来的逻辑：加载所有 attachments 和 generated_assets
            if let Some(atts) = m.attachments.as_ref() {
                let loaded = Self::load_llm_attachments(atts)?;
                tracing::info!(
                    "[build_message_request] Loaded {} attachments",
                    loaded.len()
                );
                attachments.extend(loaded);
            }
            if let Some(generated_assets) = m.generated_assets.as_ref() {
                let loaded = Self::load_llm_attachments(generated_assets)?;
                tracing::info!(
                    "[build_message_request] Loaded {} generated_assets as attachments",
                    loaded.len()
                );
                attachments.extend(loaded);
            }

            request_messages.push(LlmMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                reasoning: m.reasoning.clone(),
                // 转换 MessageToolCall 为 LlmToolCall (去除业务字段)
                tool_calls: m
                    .tool_calls
                    .as_ref()
                    .map(|calls| calls.iter().map(|tc| tc.to_llm_tool_call()).collect()),
                tool_call_id: m.tool_call_id.clone(),
                attachments: if attachments.is_empty() {
                    None
                } else {
                    Some(attachments)
                },
            });
        }

        // 8. 构建请求
        Ok(MessageRequest {
            chat_id: Some(chat_id.to_string()),
            model_id: message_config.model_id.clone().unwrap_or_default(),
            provider_id: message_config.provider_id.clone().unwrap_or_default(),
            messages: request_messages,
        })
    }

    /// 获取消息
    pub async fn get_messages(
        &self,
        chat_id: UUID,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>, AppError> {
        tracing::info!(
            "[MessageService::get_messages] Getting messages for chat_id: {}",
            chat_id
        );
        let limit = limit.unwrap_or(500);
        let offset = offset.unwrap_or(0);

        let messages = self
            .repository
            .get_messages_by_chat(&chat_id, limit, offset)
            .await
            .map_err(|e| {
                let error = format!("Failed to get messages: {}", e);
                tracing::error!(
                    "[MessageService::get_messages] Database error for chat_id {}: {}",
                    chat_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::get_messages] Retrieved {} messages for chat_id: {}",
            messages.len(),
            chat_id
        );
        Ok(messages)
    }

    /// 获取单条消息
    pub async fn get_message(&self, message_id: UUID) -> Result<Message, AppError> {
        tracing::info!(
            "[MessageService::get_message] Getting message: {}",
            message_id
        );

        match self.repository.get_message_by_id(&message_id).await? {
            Some(message) => Ok(message),
            None => {
                let error = format!("Message not found: {}", message_id);
                tracing::warn!("[MessageService::get_message] {}", error);
                Err(AppError::not_found(&error))
            }
        }
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: UUID,
        content: String,
    ) -> Result<Message, AppError> {
        tracing::info!(
            "[MessageService::update_message] Updating message: {}",
            message_id
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 先检查消息是否存在
        let _existing = self.get_message(message_id.clone()).await?;

        // 更新消息内容
        self.repository
            .update_message_content(&message_id, &content, now)
            .await
            .map_err(|e| {
                let error = format!("Failed to update message: {}", e);
                tracing::error!(
                    "[MessageService::update_message] Database error for message_id {}: {}",
                    message_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::update_message] Successfully updated message: {}",
            message_id
        );

        // 返回更新后的消息
        self.get_message(message_id).await
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: UUID) -> Result<(), AppError> {
        tracing::info!(
            "[MessageService::delete_message] Deleting message: {}",
            message_id
        );

        // 先检查消息是否存在
        self.get_message(message_id.clone()).await?;

        // 删除消息
        self.repository
            .delete_message(&message_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to delete message: {}", e);
                tracing::error!(
                    "[MessageService::delete_message] Database error for message_id {}: {}",
                    message_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::delete_message] Successfully deleted message: {}",
            message_id
        );

        Ok(())
    }

    /// 更新消息中指定工具调用的执行状态
    async fn update_tool_call_status(
        &self,
        message_id: &str,
        status_map: &HashMap<String, MessageToolExecutionStatus>,
    ) -> Result<Vec<MessageToolCall>, AppError> {
        // 1. 获取消息
        let mut message = self.get_message(message_id.to_string()).await?;

        let mut updated_calls = Vec::new();

        // 2. 更新工具调用状态
        if let Some(tool_calls) = &mut message.tool_calls {
            for tool_call in tool_calls.iter_mut() {
                if let Some(status) = status_map.get(&tool_call.id) {
                    tool_call.execution_status = status.clone();

                    // 非完成状态时清除旧结果，避免残留
                    if !matches!(
                        status,
                        MessageToolExecutionStatus::Completed | MessageToolExecutionStatus::Failed
                    ) {
                        tool_call.result = None;
                    }

                    updated_calls.push(tool_call.clone());
                }
            }
        }

        if updated_calls.is_empty() {
            return Ok(updated_calls);
        }

        // 3. 保存更新后的工具调用
        let now = Utc::now().timestamp_millis();
        self.repository
            .update_message_tools(&message.id, message.tool_calls.as_ref(), now)
            .await?;

        tracing::debug!(
            "[MessageService::update_tool_call_status] Updated {} tool calls in message {}",
            updated_calls.len(),
            message_id
        );

        Ok(updated_calls)
    }

    /// 更新消息中指定工具调用的执行结果，并根据结果调整状态
    async fn update_tool_call_results(
        &self,
        message_id: &str,
        results: &HashMap<String, String>,
    ) -> Result<Vec<MessageToolCall>, AppError> {
        let mut message = self.get_message(message_id.to_string()).await?;
        let mut updated_calls = Vec::new();

        if let Some(tool_calls) = &mut message.tool_calls {
            for tool_call in tool_calls.iter_mut() {
                if let Some(result) = results.get(&tool_call.id) {
                    tool_call.result = Some(result.clone());
                    tool_call.execution_status = if Self::is_failure_result(result) {
                        MessageToolExecutionStatus::Failed
                    } else {
                        MessageToolExecutionStatus::Completed
                    };
                    updated_calls.push(tool_call.clone());
                }
            }
        }

        if updated_calls.is_empty() {
            return Ok(updated_calls);
        }

        let now = Utc::now().timestamp_millis();
        self.repository
            .update_message_tools(&message.id, message.tool_calls.as_ref(), now)
            .await?;

        tracing::debug!(
            "[MessageService::update_tool_call_results] Updated {} tool results in message {}",
            updated_calls.len(),
            message_id
        );

        Ok(updated_calls)
    }

    fn is_failure_result(result: &str) -> bool {
        let lowered = result.to_ascii_lowercase();
        lowered.starts_with("工具执行失败")
            || lowered.starts_with("tool execution failed")
            || lowered.starts_with("error:")
    }

    /// 删除消息之后的所有消息（用于手动工具执行前的清理）
    pub async fn delete_messages_after(
        &self,
        chat_id: UUID,
        message_id: UUID,
    ) -> Result<Vec<String>, AppError> {
        tracing::info!(
            "[MessageService::delete_messages_after] Deleting messages after message_id {} in chat {}",
            message_id,
            chat_id
        );

        let deleted_message_ids = self
            .repository
            .delete_messages_after(&chat_id, &message_id)
            .await?;

        tracing::info!(
            "[MessageService::delete_messages_after] Deleted {} messages",
            deleted_message_ids.len()
        );

        Ok(deleted_message_ids)
    }

    /// 根据聊天信息构建消息配置
    fn message_config_from_chat(chat: &Session) -> MessageConfig {
        fn normalize_str(value: &Option<String>) -> Option<String> {
            value.as_ref().and_then(|text| {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else if trimmed.len() == text.len() {
                    Some(text.clone())
                } else {
                    Some(trimmed.to_string())
                }
            })
        }

        /// 过滤掉无效的数值参数（0 或负数）
        /// 这些参数如果为 0，说明它们没有被设置或不被支持，应该保持为 None
        fn normalize_numeric<T>(value: Option<T>) -> Option<T>
        where
            T: PartialOrd + Default,
        {
            value.filter(|v| *v > T::default())
        }

        let system_prompt = normalize_str(&chat.system_prompt);
        let model_id = normalize_str(&chat.model_id);
        let provider_id = normalize_str(&chat.provider_id);
        let mcp_servers = if chat.mcp_servers.is_empty() {
            None
        } else {
            Some(chat.mcp_servers.clone())
        };

        MessageConfig {
            temperature: normalize_numeric(chat.temperature),
            top_p: normalize_numeric(chat.top_p),
            top_k: normalize_numeric(chat.top_k),
            max_tokens: normalize_numeric(chat.max_tokens),
            stream: chat.stream,
            model_id,
            provider_id,
            system_prompt,
            mcp_servers,
            turn_count: chat.turn_count,
            reasoning: chat.reasoning.clone(),
        }
    }

    /// 创建一个包装的 end_callback，在调用原始回调前先将消息保存到数据库
    ///
    /// 这个辅助方法用于在流式响应结束时：
    /// 1. 异步保存助手消息到数据库
    /// 2. 然后调用原始的 end_callback
    fn wrap_end_callback_with_save(
        repository: MessageRepository,
        chat_id: String,
        config: Option<MessageConfig>,
        turn_id: Option<i32>,
        mut end_callback: impl StreamEndCallback,
    ) -> impl FnMut(String, MessageResponse) + Send + 'static {
        move |stream_id: String, response: MessageResponse| {
            // 保存助手消息到数据库
            let repository = repository.clone();
            let chat_id = chat_id.clone();
            let config = config.clone();
            let turn_id = turn_id.clone();
            let response_clone = response.clone();
            let stream_id_clone = stream_id.clone();

            tokio::spawn(async move {
                let now = Utc::now().timestamp_millis();

                tracing::info!(
                    "[MessageService::wrap_end_callback_with_save] Saving assistant message - content: {} chars, reasoning: {:?}",
                    response_clone.content.len(),
                    response_clone.reasoning.as_ref().map(|r| format!("{} chars", r.len()))
                );

                match repository
                    .create_message(&Message {
                        id: response_clone.message_id.clone(),
                        session_id: chat_id.to_string(),
                        role: LlmMessageRole::Assistant,
                        content: response_clone.content.clone(),
                        reasoning: response_clone.reasoning.clone(),
                        tool_calls: response_clone.tool_calls.clone(),
                        config,
                        turn_id,
                        tool_call_id: None,
                        attachments: None,
                        generated_assets: response_clone.generated_assets.clone(),
                        input_tokens: response_clone.input_tokens,
                        output_tokens: response_clone.output_tokens,
                        total_tokens: response_clone.total_tokens,
                        start_time: Some(now),
                        end_time: Some(now),
                        duration: response_clone.duration,
                        created_at: now,
                        updated_at: now,
                    })
                    .await
                {
                    Ok(_) => {
                        tracing::info!(
                            "[MessageService] Assistant message saved with ID: {}, reasoning included: {}",
                            response_clone.message_id,
                            response_clone.reasoning.is_some()
                        );
                    }
                    Err(e) => {
                        tracing::error!("[MessageService] Failed to save assistant message: {}", e);
                    }
                }
            });

            // 调用原始的 end_callback
            end_callback(stream_id_clone, response);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelParameters, UserMessageSendRequest};
    use crate::services::{McpService, ProviderService, SessionService, StorageService};
    use crate::storage::types::MessageConfig;
    use crate::storage::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        )
    }

    async fn setup_services() -> (Arc<SessionService>, MessageService, String) {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let mcp_service = Arc::new(McpService::new(db.clone()));
        let chat_service = Arc::new(SessionService::new(db.clone(), provider_service.clone()));
        let storage_dir = TempDir::new().expect("Failed to create temp storage dir");
        let storage_path = storage_dir.path().to_path_buf();
        let storage_service =
            Arc::new(StorageService::new(storage_path).expect("Failed to init storage service"));
        std::mem::forget(storage_dir);
        let message_service = MessageService::new(
            db,
            provider_service,
            chat_service.clone(),
            mcp_service,
            storage_service,
        );

        let chat = chat_service
            .create_chat(
                "Test Chat".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        (chat_service, message_service, chat.id)
    }

    #[tokio::test]
    async fn creates_service_successfully() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let mcp_service = Arc::new(McpService::new(db.clone()));
        let chat_service = Arc::new(SessionService::new(db.clone(), provider_service.clone()));
        let storage_dir = TempDir::new().expect("Failed to create temp storage dir");
        let storage_path = storage_dir.path().to_path_buf();
        let storage_service =
            Arc::new(StorageService::new(storage_path).expect("Failed to init storage service"));
        std::mem::forget(storage_dir);
        let _service = MessageService::new(
            db,
            provider_service,
            chat_service,
            mcp_service,
            storage_service,
        );
    }

    #[tokio::test]
    async fn send_message_requires_chat_id() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;

        let request = UserMessageSendRequest {
            chat_id: _chat_id,
            content: "test".to_string(),
            temp_user_message_id: "test".to_string(),
            attachments: None,
        };

        let err = message_service
            .send_user_message(request)
            .await
            .expect_err("expected validation error");

        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn get_message_returns_not_found() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;

        let err = message_service
            .get_message("nonexistent_message".to_string())
            .await
            .expect_err("expected not found");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    #[ignore = "requires provider setup"]
    async fn send_message_with_provider_integration() {
        let (_chat_service, _message_service, _chat_id) = setup_services().await;
    }

    #[test]
    fn message_config_serialization_roundtrip() {
        let config = MessageConfig {
            temperature: Some(0.8),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(1000),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: Some(vec![crate::storage::types::McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }]),
            turn_count: Some(5),
            reasoning: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: MessageConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.temperature, config.temperature);
        assert_eq!(roundtrip.model_id, config.model_id);
        assert_eq!(roundtrip.provider_id, config.provider_id);
        assert_eq!(roundtrip.top_k, config.top_k);
    }

    #[test]
    fn model_parameters_default_values() {
        let defaults = ModelParameters::default();

        assert_eq!(defaults.temperature, Some(0.7));
        assert_eq!(defaults.top_p, Some(0.9));
        assert_eq!(defaults.max_tokens, Some(2048));
        assert_eq!(defaults.context_length, Some(4096));
        assert_eq!(defaults.stream, Some(true));
    }

    #[test]
    fn message_config_from_chat_filters_zero_values() {
        let chat = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Chat".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: Some(0.0), // 应该被过滤掉
            top_p: Some(0.0),       // 应该被过滤掉
            top_k: Some(0),         // 应该被过滤掉
            max_tokens: Some(0),    // 应该被过滤掉
            stream: Some(false),
            model_id: Some("gpt-5-nano".to_string()),
            provider_id: Some("test-provider".to_string()),
            system_prompt: Some("Test prompt".to_string()),
            mcp_servers: vec![],
            turn_count: Some(5),
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            generative_ui: None,
            genui_spec: None,
            created_at: 0,
            updated_at: 0,
        };

        let config = MessageService::message_config_from_chat(&chat);

        // 验证 0 值被过滤为 None
        assert_eq!(config.temperature, None);
        assert_eq!(config.top_p, None);
        assert_eq!(config.max_tokens, None);

        // 验证其他字段正常保留
        assert_eq!(config.stream, Some(false));
        assert_eq!(config.model_id, Some("gpt-5-nano".to_string()));
        assert_eq!(config.provider_id, Some("test-provider".to_string()));
        assert_eq!(config.turn_count, Some(5));

        // 验证 JSON 序列化不包含 0 值
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"temperature\""));
        assert!(!json.contains("\"topP\""));
        assert!(!json.contains("\"maxTokens\""));
    }

    #[test]
    fn message_config_from_chat_preserves_valid_values() {
        let chat = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Chat".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            mcp_servers: vec![],
            turn_count: Some(10),
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            generative_ui: None,
            genui_spec: None,
            created_at: 0,
            updated_at: 0,
        };

        let config = MessageService::message_config_from_chat(&chat);

        // 验证有效值被保留
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.top_p, Some(0.9));
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.stream, Some(true));

        // 验证 JSON 序列化包含有效值
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"temperature\""));
        assert!(json.contains("\"topP\""));
        assert!(json.contains("\"maxTokens\""));
    }

    /// Verifies the cancellation pin actually fires when a registered token
    /// is looked up by `stream_id`. This is the structural assertion behind
    /// UT-DISSOLVE-003: `MessageService::cancel_stream` must propagate to
    /// the very same `CancellationToken` instance that `call_llm_api_stream`
    /// installed on `ChatOptions.signal`.
    #[tokio::test]
    async fn cancel_stream_cancels_registered_token() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;
        let token = CancellationToken::new();
        message_service
            .register_test_token("sid-1".to_string(), token.clone())
            .await;

        message_service.cancel_stream("sid-1").await;

        assert!(
            token.is_cancelled(),
            "cancel_stream must fire .cancel() on the registered token"
        );
    }

    /// Unknown / already-finished `stream_id`s must not panic — the documented
    /// contract is silent Ok, since racing a natural Done event against a Stop
    /// click is normal and the user-visible behavior is the same either way.
    #[tokio::test]
    async fn cancel_stream_unknown_id_is_silent_ok() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;
        // Must not panic; no assertion needed beyond returning normally.
        message_service.cancel_stream("does-not-exist").await;
    }

    /// A deterministic substring of the embedded generative-UI prompt. Drawn from
    /// a component description in `resources/generative-ui-prompt.txt`, so it is
    /// present iff the catalog prompt was injected.
    const CATALOG_MARKER: &str = "A help icon that reveals 'content' in a hover popover.";

    /// Count how many of the produced messages carry the catalog prompt marker.
    fn catalog_messages(messages: &[LlmMessage]) -> usize {
        messages
            .iter()
            .filter(|m| m.content.contains(CATALOG_MARKER))
            .count()
    }

    // VAL-INJECT-001 (unit): generative_ui = Some(true) injects the catalog prompt.
    #[test]
    fn build_system_messages_injects_when_generative_ui_on() {
        let messages = MessageService::build_system_messages(&None, Some(true), None);
        assert_eq!(
            catalog_messages(&messages),
            1,
            "exactly one catalog-prompt System message must be present"
        );
        assert!(messages.iter().any(
            |m| matches!(m.role, LlmMessageRole::System) && m.content.contains(CATALOG_MARKER)
        ));
    }

    // VAL-INJECT-002 (unit): None and Some(false) never inject the catalog prompt.
    #[test]
    fn build_system_messages_skips_when_generative_ui_off_or_null() {
        let none = MessageService::build_system_messages(&None, None, None);
        assert_eq!(catalog_messages(&none), 0, "None must not inject");
        assert!(none.is_empty(), "no system_prompt + None => no messages");

        let off = MessageService::build_system_messages(&None, Some(false), None);
        assert_eq!(catalog_messages(&off), 0, "Some(false) must not inject");
        assert!(
            off.is_empty(),
            "no system_prompt + Some(false) => no messages"
        );
    }

    // VAL-INJECT-006 (unit): user system_prompt and the catalog prompt coexist;
    // the user prompt is preserved intact, never overwritten or truncated.
    #[test]
    fn build_system_messages_preserves_user_prompt_alongside_catalog() {
        let user_prompt = "You are a meticulous assistant.".to_string();
        let messages =
            MessageService::build_system_messages(&Some(user_prompt.clone()), Some(true), None);

        // User prompt present and byte-identical.
        assert!(
            messages
                .iter()
                .any(|m| matches!(m.role, LlmMessageRole::System) && m.content == user_prompt),
            "the user system_prompt must be present unchanged"
        );
        // Catalog prompt also present.
        assert_eq!(
            catalog_messages(&messages),
            1,
            "the catalog prompt must coexist with the user prompt"
        );
    }

    // A blank user system_prompt is dropped (as before); only the catalog prompt
    // survives when generative_ui is on.
    #[test]
    fn build_system_messages_drops_blank_user_prompt() {
        let messages =
            MessageService::build_system_messages(&Some("   ".to_string()), Some(true), None);
        assert_eq!(messages.len(), 1, "blank user prompt is dropped");
        assert_eq!(catalog_messages(&messages), 1);
    }

    // VAL-INJECT-003 (unit): each call builds a fresh request; the catalog prompt
    // never stacks across repeated calls.
    #[test]
    fn build_system_messages_is_idempotent_no_doubling() {
        let first = MessageService::build_system_messages(&None, Some(true), None);
        let second = MessageService::build_system_messages(&None, Some(true), None);
        assert_eq!(catalog_messages(&first), 1);
        assert_eq!(catalog_messages(&second), 1);
        assert_eq!(
            first.len(),
            second.len(),
            "repeated calls must yield the same message set, never a doubled prompt"
        );
    }

    /// A sentinel embedded in the example spec; present iff the example was injected.
    const EXAMPLE_MARKER: &str = "demo-card-sentinel";

    /// Count messages that carry the genui example (preamble + the example spec).
    fn example_messages(messages: &[LlmMessage]) -> usize {
        messages
            .iter()
            .filter(|m| {
                m.content.contains(GENERATIVE_UI_EXAMPLE_PREAMBLE)
                    && m.content.contains(EXAMPLE_MARKER)
            })
            .count()
    }

    // generative_ui on + a non-blank example -> the example is injected as its own
    // System message, after the catalog prompt, both coexisting.
    #[test]
    fn build_system_messages_injects_example_after_catalog_when_on() {
        let example = format!(r#"{{"root":"{EXAMPLE_MARKER}","elements":{{}}}}"#);
        let messages =
            MessageService::build_system_messages(&None, Some(true), Some(example.as_str()));

        assert_eq!(catalog_messages(&messages), 1, "catalog must be present");
        assert_eq!(
            example_messages(&messages),
            1,
            "example must be injected once"
        );
        assert_eq!(
            messages.len(),
            2,
            "no user prompt => exactly catalog + example"
        );

        // 范例必须排在目录提示之后。
        let catalog_idx = messages
            .iter()
            .position(|m| m.content.contains(CATALOG_MARKER))
            .expect("catalog present");
        let example_idx = messages
            .iter()
            .position(|m| m.content.contains(EXAMPLE_MARKER))
            .expect("example present");
        assert!(
            example_idx > catalog_idx,
            "example must come after the catalog prompt"
        );
    }

    // An example without generative_ui is never injected: off / null gate it out
    // exactly like the catalog prompt.
    #[test]
    fn build_system_messages_ignores_example_when_generative_ui_off_or_null() {
        let example = format!(r#"{{"root":"{EXAMPLE_MARKER}","elements":{{}}}}"#);

        let off = MessageService::build_system_messages(&None, Some(false), Some(example.as_str()));
        assert_eq!(
            example_messages(&off),
            0,
            "Some(false) must not inject example"
        );
        assert!(off.is_empty(), "off + no user prompt => no messages");

        let null = MessageService::build_system_messages(&None, None, Some(example.as_str()));
        assert_eq!(example_messages(&null), 0, "None must not inject example");
        assert!(null.is_empty(), "None + no user prompt => no messages");
    }

    // A blank example is dropped even when generative_ui is on: only the catalog
    // prompt survives (mirrors the blank-user-prompt handling).
    #[test]
    fn build_system_messages_drops_blank_example() {
        let messages = MessageService::build_system_messages(&None, Some(true), Some("   "));
        assert_eq!(catalog_messages(&messages), 1);
        assert_eq!(example_messages(&messages), 0, "blank example is dropped");
        assert_eq!(messages.len(), 1, "only the catalog prompt remains");
    }
}
