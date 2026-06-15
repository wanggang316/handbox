// 服务层模块
pub mod agent;
pub mod agent_jsonl_store;
pub mod agent_migration;
pub mod agent_permission;
pub mod agent_project;
pub mod agent_run_types;
pub mod agent_session;
pub mod agent_tools;
pub mod artifact;
pub mod auth;
pub mod catalog_sync;
pub mod chat_engine;
pub mod coding_agent_runtime;
pub mod coding_agent_session;
pub mod hand_ai_catalog;
pub mod job;
pub mod mcp;
pub mod message;
pub mod model;
pub mod provider;
pub mod search;
pub mod selection;
pub mod session;
pub mod settings;
pub mod skill_service;
pub mod skills;
pub mod storage;
pub mod user_session;
pub mod word;

// 重新导出服务
pub use crate::storage::Database;
pub use agent::{AgentParameter, AgentService};
pub use agent_migration::{
    migrate_and_drop_legacy_if_present, migrate_sqlite_sessions_to_jsonl, MigrateAndDropReport,
    MigrationReport,
};
pub use agent_project::AgentProjectService;
pub use agent_run_types::{AgentRunAttachment, AgentRunRequest};
pub use agent_session::{AgentSessionParameter, AgentSessionService};
pub use artifact::ArtifactService;
pub use auth::GoogleOAuthService;
pub use coding_agent_runtime::{
    abort_run, drive_agent_run, images_from_attachments, steer_run, CodingRunSink, RunDriveHandles,
};
pub use job::{JobCreateRequest, JobService, JobUpdateRequest};
pub use mcp::McpService;
pub use message::MessageService;
pub use model::ModelService;
pub use provider::ProviderService;
pub use search::SearchService;
pub use selection::setup_selection;
pub use session::{SessionParameter, SessionService};
pub use settings::SettingsService;
pub use skill_service::SkillService;
pub use storage::StorageService;
pub use user_session::UserSessionService;
pub use word::WordService;
