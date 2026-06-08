// 服务层模块
pub mod agent;
pub mod agent_runtime;
pub mod agent_session;
pub mod artifact;
pub mod auth;
pub mod catalog_sync;
pub mod chat_engine;
pub mod hand_ai_catalog;
pub mod mcp;
pub mod message;
pub mod model;
pub mod provider;
pub mod search;
pub mod selection;
pub mod session;
pub mod settings;
pub mod storage;
pub mod user_session;
pub mod word;

// 重新导出服务
pub use crate::storage::Database;
pub use agent::{AgentParameter, AgentService};
pub use agent_runtime::{AgentRunRequest, AgentRuntime, RunSink};
pub use agent_session::{AgentSessionParameter, AgentSessionService};
pub use artifact::ArtifactService;
pub use auth::GoogleOAuthService;
pub use session::{SessionParameter, SessionService};
pub use mcp::McpService;
pub use message::MessageService;
pub use model::ModelService;
pub use provider::ProviderService;
pub use search::SearchService;
pub use settings::SettingsService;
pub use selection::setup_selection;
pub use storage::StorageService;
pub use user_session::UserSessionService;
pub use word::WordService;
