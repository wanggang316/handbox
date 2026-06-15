// Scheduled-job domain types.
//
// Mirrors the `jobs` and `job_executions` tables (migrations 049/050).
// `Job` holds a structured `JobTarget` so the executor gets a typed target;
// the repository maps it to/from the `target_kind` + `target_config` columns
// via `JobTarget::into_db_parts` / `JobTarget::from_db_parts`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Timestamp, UUID};

/// What a job runs when it fires.
///
/// Internally tagged by `kind`; the tag values (`artifact` / `agent` /
/// `prompt`) are stored in `jobs.target_kind`, and the remaining fields are
/// stored as JSON in `jobs.target_config`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum JobTarget {
    /// Run an installed Artifact.
    Artifact {
        artifact_id: UUID,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// Send an initial message to an Agent (optionally scoped to a project).
    Agent {
        agent_id: UUID,
        initial_message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project_id: Option<UUID>,
    },
    /// Send a one-shot prompt to a provider/model.
    Prompt {
        provider_id: String,
        model_id: String,
        prompt: String,
        #[serde(default)]
        session_strategy: SessionStrategy,
    },
}

/// How a `Prompt` target manages its chat session. Currently a new session is
/// created for every run; the enum exists so M-later features can extend it
/// without changing the stored JSON shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStrategy {
    /// Create a fresh session on each run.
    #[default]
    NewSession,
}

impl JobTarget {
    /// The `target_kind` column value for this variant.
    pub fn kind(&self) -> &'static str {
        match self {
            JobTarget::Artifact { .. } => "artifact",
            JobTarget::Agent { .. } => "agent",
            JobTarget::Prompt { .. } => "prompt",
        }
    }

    /// Split into the two DB columns: `(target_kind, target_config_json)`.
    /// `target_config` is the variant's fields serialized *without* the `kind`
    /// tag, matching the table comment in migration 049.
    pub fn into_db_parts(&self) -> Result<(String, String), serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("kind");
        }
        let config_json = serde_json::to_string(&value)?;
        Ok((self.kind().to_string(), config_json))
    }

    /// Rebuild a `JobTarget` from the two DB columns.
    pub fn from_db_parts(
        target_kind: &str,
        target_config_json: &str,
    ) -> Result<Self, serde_json::Error> {
        let mut value: serde_json::Value = serde_json::from_str(target_config_json)?;
        let obj = value.as_object_mut().ok_or_else(|| {
            serde::de::Error::custom("target_config must be a JSON object")
        })?;
        obj.insert(
            "kind".to_string(),
            serde_json::Value::String(target_kind.to_string()),
        );
        serde_json::from_value(value)
    }
}

/// A scheduled-job definition (one row in `jobs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: UUID,
    pub name: String,
    pub description: Option<String>,
    /// Structured target; maps to `target_kind` + `target_config` columns.
    pub target: JobTarget,
    pub cron_expr: String,
    pub timezone: String,
    pub enabled: bool,
    pub last_run_at: Option<Timestamp>,
    pub next_run_at: Option<Timestamp>,
    pub last_status: Option<ExecutionStatus>,
    pub run_count: i32,
    pub failure_count: i32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Outcome of a single job run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Timeout,
}

/// What initiated a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Schedule,
    Manual,
}

/// A single execution record (one row in `job_executions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecution {
    pub id: UUID,
    pub job_id: UUID,
    pub status: ExecutionStatus,
    pub trigger: Trigger,
    pub attempt: i32,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub result_ref: Option<String>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub duration: Option<i64>, // milliseconds
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(target: &JobTarget) -> JobTarget {
        let json = serde_json::to_string(target).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn job_target_artifact_roundtrip() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        let target = JobTarget::Artifact {
            artifact_id: "artifact_1".to_string(),
            args: vec!["--flag".to_string(), "x".to_string()],
            env,
        };
        assert_eq!(roundtrip(&target), target);

        // Tag and camelCase field naming appear in the wire form.
        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["kind"], "artifact");
        assert!(json.get("artifactId").is_some());
    }

    #[test]
    fn job_target_agent_roundtrip() {
        let target = JobTarget::Agent {
            agent_id: "agent_1".to_string(),
            initial_message: "hello".to_string(),
            project_id: Some("project_1".to_string()),
        };
        assert_eq!(roundtrip(&target), target);

        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["kind"], "agent");
        assert_eq!(json["initialMessage"], "hello");
        assert_eq!(json["projectId"], "project_1");
    }

    #[test]
    fn job_target_prompt_roundtrip() {
        let target = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "Summarize today".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        assert_eq!(roundtrip(&target), target);

        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["kind"], "prompt");
        assert_eq!(json["providerId"], "openai");
        assert_eq!(json["sessionStrategy"], "new_session");
    }

    #[test]
    fn job_target_optional_fields_default() {
        // Artifact with no args/env, agent with no project.
        let artifact = JobTarget::from_db_parts("artifact", r#"{"artifactId":"a1"}"#)
            .expect("from_db_parts");
        assert_eq!(
            artifact,
            JobTarget::Artifact {
                artifact_id: "a1".to_string(),
                args: vec![],
                env: HashMap::new(),
            }
        );

        let agent = JobTarget::from_db_parts(
            "agent",
            r#"{"agentId":"ag1","initialMessage":"hi"}"#,
        )
        .expect("from_db_parts");
        assert_eq!(
            agent,
            JobTarget::Agent {
                agent_id: "ag1".to_string(),
                initial_message: "hi".to_string(),
                project_id: None,
            }
        );
    }

    #[test]
    fn job_target_db_parts_roundtrip() {
        let mut env = HashMap::new();
        env.insert("A".to_string(), "1".to_string());
        let targets = vec![
            JobTarget::Artifact {
                artifact_id: "a1".to_string(),
                args: vec!["x".to_string()],
                env,
            },
            JobTarget::Agent {
                agent_id: "ag1".to_string(),
                initial_message: "go".to_string(),
                project_id: None,
            },
            JobTarget::Prompt {
                provider_id: "p1".to_string(),
                model_id: "m1".to_string(),
                prompt: "do it".to_string(),
                session_strategy: SessionStrategy::NewSession,
            },
        ];

        for target in &targets {
            let (kind, config) = target.into_db_parts().expect("into_db_parts");
            assert_eq!(kind, target.kind());
            // `target_config` must not carry the discriminant tag.
            assert!(!config.contains("\"kind\""));
            let restored = JobTarget::from_db_parts(&kind, &config).expect("from_db_parts");
            assert_eq!(&restored, target);
        }
    }

    #[test]
    fn execution_status_and_trigger_wire_form() {
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&Trigger::Schedule).unwrap(),
            "\"schedule\""
        );
    }

    #[test]
    fn job_roundtrip() {
        let job = Job {
            id: "job_1".to_string(),
            name: "Daily report".to_string(),
            description: Some("Runs every morning".to_string()),
            target: JobTarget::Prompt {
                provider_id: "openai".to_string(),
                model_id: "gpt-4".to_string(),
                prompt: "report".to_string(),
                session_strategy: SessionStrategy::NewSession,
            },
            cron_expr: "0 9 * * *".to_string(),
            timezone: "local".to_string(),
            enabled: true,
            last_run_at: Some(1000),
            next_run_at: Some(2000),
            last_status: Some(ExecutionStatus::Success),
            run_count: 3,
            failure_count: 1,
            created_at: 100,
            updated_at: 200,
        };

        let json = serde_json::to_string(&job).expect("serialize");
        let restored: Job = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, job.id);
        assert_eq!(restored.target, job.target);
        assert_eq!(restored.last_status, job.last_status);
    }

    #[test]
    fn job_execution_roundtrip() {
        let exec = JobExecution {
            id: "exec_1".to_string(),
            job_id: "job_1".to_string(),
            status: ExecutionStatus::Running,
            trigger: Trigger::Manual,
            attempt: 1,
            stdout: Some("out".to_string()),
            stderr: None,
            exit_code: Some(0),
            error: None,
            result_ref: Some("session_1".to_string()),
            started_at: 1000,
            ended_at: Some(1500),
            duration: Some(500),
            created_at: 1000,
        };

        let json = serde_json::to_string(&exec).expect("serialize");
        let restored: JobExecution = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, exec.id);
        assert_eq!(restored.status, exec.status);
        assert_eq!(restored.trigger, exec.trigger);
        assert_eq!(restored.duration, exec.duration);
    }
}
