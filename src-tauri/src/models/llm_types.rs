// Re-export the leaf types HandBox storage and services use, sourced from
// `handbox-llm` during the dissolve-handbox-llm transition. As long as this
// file `pub use`s the types, `crate::models::llm_types::X` and
// `handbox_llm::types::X` are the SAME nominal type — import sites in
// HandBox app code reach into the new path without forcing every aggregate
// (LlmMessage, LlmRequest, ...) to migrate in lockstep. M3-T0 flips this
// module from re-exports to verbatim copies just before the crate is
// deleted, restoring the original M1-T1 definitions to a self-contained
// home.

pub use handbox_llm::types::{
    LlmMessageAttachment, LlmMessageRole, LlmModelParameter, LlmReasoningEffort,
    LlmReasoningEffortConfig, LlmReasoningSummary, LlmResponsesReasoning, LlmThinkingConfig,
    LlmToolCall, LlmToolFunction, ModelPricing,
};
