// Scheduled-job 数据访问层
//
// 两个仓储：
// - `JobRepository`：`jobs` 表 CRUD + due-selection 查询 + 运行后统计字段更新。
// - `JobExecutionRepository`：插入 running 行、原地更新到终态、按 job 列出、
//   FIFO 裁剪到最近 N 行、启动时 reconcile 残留 running 行。
//
// 这是 commands / executor / scheduler 的数据基础。本模块只提供方法 + 单测；
// 「写入即 prune」「启动即 reconcile」的接线分别属 history-pruning / scheduler。

use crate::models::AppError;
use crate::storage::types::{ExecutionStatus, Job, JobExecution, JobTarget, Timestamp, Trigger};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// `job_executions` 默认 FIFO 历史上限：每个 job 保留最近 N 行执行记录。
pub const DEFAULT_EXECUTION_HISTORY_LIMIT: i64 = 100;

/// `ExecutionStatus` 的 `last_status` / `status` 列字符串表示。
///
/// 与 `job.rs` 的 serde `snake_case` 线格式一致，但在仓储层显式映射，避免
/// 依赖 serde_json 的引号包裹，并把 DB 列字符串约定收敛在数据访问层。
fn execution_status_as_str(status: ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Running => "running",
        ExecutionStatus::Success => "success",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Timeout => "timeout",
    }
}

/// 把 DB 列字符串解析回 `ExecutionStatus`，未知值视为数据损坏返回错误，
/// 避免把无效状态静默吞掉。
fn execution_status_from_str(value: &str) -> Result<ExecutionStatus, AppError> {
    match value {
        "running" => Ok(ExecutionStatus::Running),
        "success" => Ok(ExecutionStatus::Success),
        "failed" => Ok(ExecutionStatus::Failed),
        "timeout" => Ok(ExecutionStatus::Timeout),
        other => Err(AppError::internal_error(&format!(
            "Invalid execution status in database: {}",
            other
        ))),
    }
}

/// `Trigger` 的 `trigger` 列字符串表示。
fn trigger_as_str(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Schedule => "schedule",
        Trigger::Manual => "manual",
    }
}

/// 把 DB 列字符串解析回 `Trigger`。
fn trigger_from_str(value: &str) -> Result<Trigger, AppError> {
    match value {
        "schedule" => Ok(Trigger::Schedule),
        "manual" => Ok(Trigger::Manual),
        other => Err(AppError::internal_error(&format!(
            "Invalid trigger in database: {}",
            other
        ))),
    }
}

/// Job 定义仓储层（`jobs` 表）。
#[derive(Clone)]
pub struct JobRepository {
    db: Arc<Database>,
}

impl JobRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建一个 job。`target` 拆成 `target_kind` + `target_config` 两列存储。
    pub async fn create(&self, job: &Job) -> Result<(), AppError> {
        let (target_kind, target_config) = job
            .target
            .into_db_parts()
            .map_err(|e| AppError::validation_error(&format!("Invalid job target: {}", e)))?;

        let query = r#"
            INSERT INTO jobs (
                id, name, description, target_kind, target_config, cron_expr, timezone,
                enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#;

        sqlx::query(query)
            .bind(&job.id)
            .bind(&job.name)
            .bind(&job.description)
            .bind(&target_kind)
            .bind(&target_config)
            .bind(&job.cron_expr)
            .bind(&job.timezone)
            .bind(job.enabled)
            .bind(job.last_run_at)
            .bind(job.next_run_at)
            .bind(job.last_status.map(execution_status_as_str))
            .bind(job.run_count)
            .bind(job.failure_count)
            .bind(job.created_at)
            .bind(job.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create job: {}", e)))?;

        Ok(())
    }

    /// 按 id 获取一个 job，不存在返回 `None`。
    pub async fn get(&self, id: &str) -> Result<Option<Job>, AppError> {
        let row = sqlx::query(JOB_SELECT_COLUMNS_WITH_WHERE_ID)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get job: {}", e)))?;

        row.map(Self::row_to_job).transpose()
    }

    /// 分页列出 jobs，按 `created_at` 降序（最新优先）。
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Job>, AppError> {
        let query = format!(
            "{} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            JOB_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list jobs: {}", e)))?;

        rows.into_iter().map(Self::row_to_job).collect()
    }

    /// 全量更新一个 job 的定义字段（不触碰运行统计，那由 `update_after_run`/
    /// `set_enabled` 负责）。返回的 `not_found` 用于 id 不存在的情形。
    pub async fn update(&self, job: &Job) -> Result<(), AppError> {
        let (target_kind, target_config) = job
            .target
            .into_db_parts()
            .map_err(|e| AppError::validation_error(&format!("Invalid job target: {}", e)))?;

        let query = r#"
            UPDATE jobs SET
                name = $1, description = $2, target_kind = $3, target_config = $4,
                cron_expr = $5, timezone = $6, enabled = $7, next_run_at = $8, updated_at = $9
            WHERE id = $10
        "#;

        let result = sqlx::query(query)
            .bind(&job.name)
            .bind(&job.description)
            .bind(&target_kind)
            .bind(&target_config)
            .bind(&job.cron_expr)
            .bind(&job.timezone)
            .bind(job.enabled)
            .bind(job.next_run_at)
            .bind(job.updated_at)
            .bind(&job.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update job: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", job.id)));
        }

        Ok(())
    }

    /// 删除一个 job。`job_executions` 通过 FK `ON DELETE CASCADE` 一并删除
    /// （需连接上 `PRAGMA foreign_keys = ON`，sqlx 默认开启）。
    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete job: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// 启用/禁用一个 job，并刷新 `updated_at`。
    pub async fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE jobs SET enabled = $1, updated_at = $2 WHERE id = $3")
            .bind(enabled)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to set job enabled: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// 一次运行结束后写入统计：`last_run_at` / `last_status` / `next_run_at`，
    /// 并把 `run_count` 自增 1；失败时 `failure_count` 也自增 1。
    ///
    /// `next_run_at` 由 caller（scheduler）算好后传入；本层不重算 cron。
    pub async fn update_after_run(
        &self,
        id: &str,
        last_run_at: Timestamp,
        last_status: ExecutionStatus,
        next_run_at: Option<Timestamp>,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let failure_increment: i32 = if matches!(last_status, ExecutionStatus::Success) {
            0
        } else {
            1
        };

        let query = r#"
            UPDATE jobs SET
                last_run_at = $1,
                last_status = $2,
                next_run_at = $3,
                run_count = run_count + 1,
                failure_count = failure_count + $4,
                updated_at = $5
            WHERE id = $6
        "#;

        let result = sqlx::query(query)
            .bind(last_run_at)
            .bind(execution_status_as_str(last_status))
            .bind(next_run_at)
            .bind(failure_increment)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update job after run: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// 设置一个 job 的下次运行时间（caller 重算 cron 后调用）。
    pub async fn recompute_next_run(
        &self,
        id: &str,
        next_run_at: Option<Timestamp>,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("UPDATE jobs SET next_run_at = $1, updated_at = $2 WHERE id = $3")
                .bind(next_run_at)
                .bind(updated_at)
                .bind(id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to set job next_run_at: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// 返回所有到期 job：`enabled = 1 AND next_run_at <= now`，按 `next_run_at`
    /// 升序（最该跑的先返回）。命中 `idx_jobs_enabled_next`。
    ///
    /// `next_run_at IS NULL` 的 job（尚未排程）不会被选中。
    pub async fn list_due(&self, now: Timestamp) -> Result<Vec<Job>, AppError> {
        let query = format!(
            "{} WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= $1 \
             ORDER BY next_run_at ASC",
            JOB_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(now)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list due jobs: {}", e)))?;

        rows.into_iter().map(Self::row_to_job).collect()
    }

    /// 把数据库行还原为 `Job`，包括从两列重建多态 `target`。
    fn row_to_job(row: sqlx::sqlite::SqliteRow) -> Result<Job, AppError> {
        let target_kind: String = row
            .try_get("target_kind")
            .map_err(|e| AppError::internal_error(&format!("Failed to read target_kind: {}", e)))?;
        let target_config: String = row.try_get("target_config").map_err(|e| {
            AppError::internal_error(&format!("Failed to read target_config: {}", e))
        })?;
        let target = JobTarget::from_db_parts(&target_kind, &target_config).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse job target: {}", e))
        })?;

        // 可空 last_status 列：NULL -> None，非空字符串解析为枚举。
        let last_status: Option<String> = row.try_get("last_status").map_err(|e| {
            AppError::internal_error(&format!("Failed to read last_status: {}", e))
        })?;
        let last_status = last_status
            .map(|s| execution_status_from_str(&s))
            .transpose()?;

        Ok(Job {
            id: row
                .try_get("id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read id: {}", e)))?,
            name: row
                .try_get("name")
                .map_err(|e| AppError::internal_error(&format!("Failed to read name: {}", e)))?,
            description: row.try_get("description").map_err(|e| {
                AppError::internal_error(&format!("Failed to read description: {}", e))
            })?,
            target,
            cron_expr: row.try_get("cron_expr").map_err(|e| {
                AppError::internal_error(&format!("Failed to read cron_expr: {}", e))
            })?,
            timezone: row.try_get("timezone").map_err(|e| {
                AppError::internal_error(&format!("Failed to read timezone: {}", e))
            })?,
            enabled: row.try_get("enabled").map_err(|e| {
                AppError::internal_error(&format!("Failed to read enabled: {}", e))
            })?,
            last_run_at: row.try_get("last_run_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read last_run_at: {}", e))
            })?,
            next_run_at: row.try_get("next_run_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read next_run_at: {}", e))
            })?,
            last_status,
            run_count: row.try_get("run_count").map_err(|e| {
                AppError::internal_error(&format!("Failed to read run_count: {}", e))
            })?,
            failure_count: row.try_get("failure_count").map_err(|e| {
                AppError::internal_error(&format!("Failed to read failure_count: {}", e))
            })?,
            created_at: row.try_get("created_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read created_at: {}", e))
            })?,
            updated_at: row.try_get("updated_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read updated_at: {}", e))
            })?,
        })
    }
}

/// `jobs` 的 SELECT 列清单（不含 WHERE/ORDER）。
const JOB_SELECT_COLUMNS: &str = r#"
    SELECT id, name, description, target_kind, target_config, cron_expr, timezone,
           enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
           created_at, updated_at
    FROM jobs
"#;

/// `jobs` 单行按 id 查询。
const JOB_SELECT_COLUMNS_WITH_WHERE_ID: &str = r#"
    SELECT id, name, description, target_kind, target_config, cron_expr, timezone,
           enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
           created_at, updated_at
    FROM jobs WHERE id = $1
"#;

/// Job 执行记录仓储层（`job_executions` 表）。
#[derive(Clone)]
pub struct JobExecutionRepository {
    db: Arc<Database>,
}

impl JobExecutionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 插入一行 `running` 执行记录（一次运行的起点），返回该行 id。
    ///
    /// 终态字段（stdout/stderr/exit_code/error/result_ref/ended_at/duration）此时
    /// 全部为 NULL，由 `finalize` 原地补齐。
    pub async fn insert_running(
        &self,
        id: &str,
        job_id: &str,
        trigger: Trigger,
        attempt: i32,
        started_at: Timestamp,
        created_at: Timestamp,
    ) -> Result<String, AppError> {
        let query = r#"
            INSERT INTO job_executions (
                id, job_id, status, trigger, attempt,
                stdout, stderr, exit_code, error, result_ref,
                started_at, ended_at, duration, created_at
            )
            VALUES ($1, $2, $3, $4, $5, NULL, NULL, NULL, NULL, NULL, $6, NULL, NULL, $7)
        "#;

        sqlx::query(query)
            .bind(id)
            .bind(job_id)
            .bind(execution_status_as_str(ExecutionStatus::Running))
            .bind(trigger_as_str(trigger))
            .bind(attempt)
            .bind(started_at)
            .bind(created_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to insert running execution: {}", e))
            })?;

        Ok(id.to_string())
    }

    /// 把同一行执行记录原地更新到终态。
    ///
    /// `status` 必须是终态之一（success/failed/timeout）；传入 `Running` 视为调用方
    /// 错误并拒绝，以免把一行「完成」回退成运行中。
    #[allow(clippy::too_many_arguments)]
    pub async fn finalize(
        &self,
        id: &str,
        status: ExecutionStatus,
        stdout: Option<&str>,
        stderr: Option<&str>,
        exit_code: Option<i32>,
        error: Option<&str>,
        result_ref: Option<&str>,
        ended_at: Timestamp,
        duration: i64,
    ) -> Result<(), AppError> {
        if matches!(status, ExecutionStatus::Running) {
            return Err(AppError::validation_error(
                "finalize requires a terminal status (success/failed/timeout)",
            ));
        }

        let query = r#"
            UPDATE job_executions SET
                status = $1, stdout = $2, stderr = $3, exit_code = $4, error = $5,
                result_ref = $6, ended_at = $7, duration = $8
            WHERE id = $9
        "#;

        let result = sqlx::query(query)
            .bind(execution_status_as_str(status))
            .bind(stdout)
            .bind(stderr)
            .bind(exit_code)
            .bind(error)
            .bind(result_ref)
            .bind(ended_at)
            .bind(duration)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to finalize execution: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Execution not found: {}", id)));
        }

        Ok(())
    }

    /// 列出一个 job 的执行记录，最新优先（按 `started_at` 降序，id 作次序稳定项）。
    pub async fn list_for_job(
        &self,
        job_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobExecution>, AppError> {
        let query = format!(
            "{} WHERE job_id = $1 ORDER BY started_at DESC, id DESC LIMIT $2 OFFSET $3",
            EXECUTION_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(job_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list executions: {}", e))
            })?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    /// 把某个 job 的执行历史 FIFO 裁剪到最近 `keep` 行（按 `started_at`）。
    ///
    /// 永不删除仍处于 `running` 的行（避免裁掉正在进行的运行）；只在已完成的行
    /// 超过上限时删除最旧的。返回被删除的行数。
    pub async fn prune_to(&self, job_id: &str, keep: i64) -> Result<u64, AppError> {
        // 选出该 job 的所有非 running 行，按 started_at 降序（最新优先），
        // 删除排在 `keep` 之后的（最旧的）。running 行完全不参与。
        let query = r#"
            DELETE FROM job_executions
            WHERE id IN (
                SELECT id FROM job_executions
                WHERE job_id = $1 AND status != 'running'
                ORDER BY started_at DESC, id DESC
                LIMIT -1 OFFSET $2
            )
        "#;

        let result = sqlx::query(query)
            .bind(job_id)
            .bind(keep)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to prune executions: {}", e))
            })?;

        Ok(result.rows_affected())
    }

    /// 启动时 reconcile：把所有残留 `running` 的执行行标记为给定终态
    /// （通常是 `Failed`，表示上次进程退出时被中断）。返回受影响行数。
    ///
    /// 仅在调用方明确写入接线（scheduler 启动）时使用；本层只提供能力。
    pub async fn reconcile_stale_running(
        &self,
        status: ExecutionStatus,
        error: &str,
        ended_at: Timestamp,
    ) -> Result<u64, AppError> {
        if matches!(status, ExecutionStatus::Running) {
            return Err(AppError::validation_error(
                "reconcile_stale_running requires a terminal status",
            ));
        }

        // duration = ended_at - started_at，确保非负。
        let query = r#"
            UPDATE job_executions SET
                status = $1,
                error = $2,
                ended_at = $3,
                duration = MAX($3 - started_at, 0)
            WHERE status = 'running'
        "#;

        let result = sqlx::query(query)
            .bind(execution_status_as_str(status))
            .bind(error)
            .bind(ended_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to reconcile running executions: {}", e))
            })?;

        Ok(result.rows_affected())
    }

    /// 把数据库行还原为 `JobExecution`。可空列以 `Option` 解码，避免把 NULL
    /// decode 进非 Option 字段而 panic。
    fn row_to_execution(row: sqlx::sqlite::SqliteRow) -> Result<JobExecution, AppError> {
        let status_str: String = row
            .try_get("status")
            .map_err(|e| AppError::internal_error(&format!("Failed to read status: {}", e)))?;
        let status = execution_status_from_str(&status_str)?;

        let trigger_str: String = row
            .try_get("trigger")
            .map_err(|e| AppError::internal_error(&format!("Failed to read trigger: {}", e)))?;
        let trigger = trigger_from_str(&trigger_str)?;

        Ok(JobExecution {
            id: row
                .try_get("id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read id: {}", e)))?,
            job_id: row
                .try_get("job_id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read job_id: {}", e)))?,
            status,
            trigger,
            attempt: row.try_get("attempt").map_err(|e| {
                AppError::internal_error(&format!("Failed to read attempt: {}", e))
            })?,
            stdout: row
                .try_get("stdout")
                .map_err(|e| AppError::internal_error(&format!("Failed to read stdout: {}", e)))?,
            stderr: row
                .try_get("stderr")
                .map_err(|e| AppError::internal_error(&format!("Failed to read stderr: {}", e)))?,
            exit_code: row.try_get("exit_code").map_err(|e| {
                AppError::internal_error(&format!("Failed to read exit_code: {}", e))
            })?,
            error: row
                .try_get("error")
                .map_err(|e| AppError::internal_error(&format!("Failed to read error: {}", e)))?,
            result_ref: row.try_get("result_ref").map_err(|e| {
                AppError::internal_error(&format!("Failed to read result_ref: {}", e))
            })?,
            started_at: row.try_get("started_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read started_at: {}", e))
            })?,
            ended_at: row.try_get("ended_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read ended_at: {}", e))
            })?,
            duration: row.try_get("duration").map_err(|e| {
                AppError::internal_error(&format!("Failed to read duration: {}", e))
            })?,
            created_at: row.try_get("created_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read created_at: {}", e))
            })?,
        })
    }
}

/// `job_executions` 的 SELECT 列清单（不含 WHERE/ORDER）。
const EXECUTION_SELECT_COLUMNS: &str = r#"
    SELECT id, job_id, status, trigger, attempt, stdout, stderr, exit_code, error,
           result_ref, started_at, ended_at, duration, created_at
    FROM job_executions
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::{Job, JobTarget, SessionStrategy};
    use std::collections::HashMap;
    use tempfile::tempdir;

    /// 建一个临时 SQLite 库并跑全部迁移（含 049/050 的 jobs/job_executions）。
    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        (db, temp_dir)
    }

    fn sample_job(id: &str, now: Timestamp) -> Job {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        Job {
            id: id.to_string(),
            name: format!("Job {}", id),
            description: Some("a scheduled job".to_string()),
            target: JobTarget::Artifact {
                artifact_id: "artifact_1".to_string(),
                args: vec!["--flag".to_string()],
                env,
            },
            cron_expr: "0 9 * * *".to_string(),
            timezone: "local".to_string(),
            enabled: true,
            last_run_at: None,
            next_run_at: Some(now + 1000),
            last_status: None,
            run_count: 0,
            failure_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_job_crud_roundtrip() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        // get round-trips including the polymorphic target.
        let fetched = repo.get("job_1").await.unwrap().expect("job exists");
        assert_eq!(fetched.id, job.id);
        assert_eq!(fetched.name, job.name);
        assert_eq!(fetched.description, job.description);
        assert_eq!(fetched.target, job.target);
        assert_eq!(fetched.cron_expr, job.cron_expr);
        assert!(fetched.enabled);
        assert_eq!(fetched.next_run_at, Some(now + 1000));
        assert_eq!(fetched.last_status, None);
        assert_eq!(fetched.run_count, 0);

        // missing id -> None.
        assert!(repo.get("nope").await.unwrap().is_none());

        // update changes definition fields.
        let mut updated = fetched.clone();
        updated.name = "Renamed".to_string();
        updated.target = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "summarize".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        updated.cron_expr = "*/5 * * * *".to_string();
        updated.next_run_at = Some(now + 2000);
        updated.updated_at = now + 50;
        repo.update(&updated).await.unwrap();

        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.name, "Renamed");
        assert_eq!(after.target, updated.target);
        assert_eq!(after.cron_expr, "*/5 * * * *");
        assert_eq!(after.next_run_at, Some(now + 2000));

        // updating a missing job is not_found.
        let mut ghost = sample_job("ghost", now);
        ghost.name = "x".to_string();
        assert!(repo.update(&ghost).await.is_err());

        // delete.
        repo.delete("job_1").await.unwrap();
        assert!(repo.get("job_1").await.unwrap().is_none());
        assert!(repo.delete("job_1").await.is_err());
    }

    #[tokio::test]
    async fn test_job_list_pagination() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        for i in 0..5 {
            let mut job = sample_job(&format!("job_{}", i), now + i);
            job.created_at = now + i;
            repo.create(&job).await.unwrap();
        }

        // newest-first by created_at.
        let page = repo.list(2, 0).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, "job_4");
        assert_eq!(page[1].id, "job_3");

        let page2 = repo.list(2, 2).await.unwrap();
        assert_eq!(page2[0].id, "job_2");
        assert_eq!(page2[1].id, "job_1");

        let total = repo.list(100, 0).await.unwrap();
        assert_eq!(total.len(), 5);
    }

    #[tokio::test]
    async fn test_set_enabled_and_recompute_next_run() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        repo.set_enabled("job_1", false, now + 10).await.unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert!(!after.enabled);
        assert_eq!(after.updated_at, now + 10);

        // recompute_next_run can set NULL (job no longer scheduled).
        repo.recompute_next_run("job_1", None, now + 20)
            .await
            .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.next_run_at, None);

        repo.recompute_next_run("job_1", Some(now + 5000), now + 30)
            .await
            .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.next_run_at, Some(now + 5000));

        assert!(repo.set_enabled("ghost", true, now).await.is_err());
        assert!(repo.recompute_next_run("ghost", None, now).await.is_err());
    }

    #[tokio::test]
    async fn test_update_after_run_counts() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        // success run: run_count +1, failure_count unchanged.
        repo.update_after_run(
            "job_1",
            now + 100,
            ExecutionStatus::Success,
            Some(now + 9000),
            now + 100,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 1);
        assert_eq!(after.failure_count, 0);
        assert_eq!(after.last_run_at, Some(now + 100));
        assert_eq!(after.last_status, Some(ExecutionStatus::Success));
        assert_eq!(after.next_run_at, Some(now + 9000));

        // failed run: both counters +1.
        repo.update_after_run(
            "job_1",
            now + 200,
            ExecutionStatus::Failed,
            None,
            now + 200,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 2);
        assert_eq!(after.failure_count, 1);
        assert_eq!(after.last_status, Some(ExecutionStatus::Failed));
        assert_eq!(after.next_run_at, None);

        // timeout counts as failure too.
        repo.update_after_run(
            "job_1",
            now + 300,
            ExecutionStatus::Timeout,
            Some(now + 9999),
            now + 300,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 3);
        assert_eq!(after.failure_count, 2);

        assert!(repo
            .update_after_run("ghost", now, ExecutionStatus::Success, None, now)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_list_due_selects_enabled_and_past_due() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let base = 1_000_000i64;
        let now = base + 5000;

        // due: enabled, next_run_at <= now.
        let mut due_a = sample_job("due_a", base);
        due_a.next_run_at = Some(now - 100);
        repo.create(&due_a).await.unwrap();

        let mut due_b = sample_job("due_b", base);
        due_b.next_run_at = Some(now); // boundary: == now is due
        repo.create(&due_b).await.unwrap();

        // not due: future next_run_at.
        let mut future = sample_job("future", base);
        future.next_run_at = Some(now + 100);
        repo.create(&future).await.unwrap();

        // not due: disabled even though past.
        let mut disabled = sample_job("disabled", base);
        disabled.enabled = false;
        disabled.next_run_at = Some(now - 100);
        repo.create(&disabled).await.unwrap();

        // not due: next_run_at IS NULL.
        let mut unscheduled = sample_job("unscheduled", base);
        unscheduled.next_run_at = None;
        repo.create(&unscheduled).await.unwrap();

        let due = repo.list_due(now).await.unwrap();
        let ids: Vec<&str> = due.iter().map(|j| j.id.as_str()).collect();
        // ordered by next_run_at ASC: due_a (now-100) before due_b (now).
        assert_eq!(ids, vec!["due_a", "due_b"]);
    }

    #[tokio::test]
    async fn test_list_due_uses_enabled_next_index() {
        // Confirm the query planner picks idx_jobs_enabled_next for the due scan.
        // EXPLAIN QUERY PLAN returns (id, parent, notused, detail); the plan
        // text lives in the `detail` column.
        let (db, _tmp) = create_test_db().await;
        let rows = sqlx::query(
            "EXPLAIN QUERY PLAN \
             SELECT id FROM jobs \
             WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= 0 \
             ORDER BY next_run_at ASC",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let plan: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").unwrap_or_default())
            .collect();
        let joined = plan.join(" ");
        assert!(
            joined.contains("idx_jobs_enabled_next"),
            "due query must use idx_jobs_enabled_next, plan was: {}",
            joined
        );
    }

    // ---- JobExecutionRepository ----

    async fn seed_job(repo: &JobRepository, id: &str, now: Timestamp) {
        repo.create(&sample_job(id, now)).await.unwrap();
    }

    #[tokio::test]
    async fn test_execution_insert_running_then_finalize_in_place() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;

        let exec_id = execs
            .insert_running("exec_1", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        assert_eq!(exec_id, "exec_1");

        // running row: terminal fields are NULL (decode into Option, no panic).
        let running = execs.list_for_job("job_1", 10, 0).await.unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].status, ExecutionStatus::Running);
        assert_eq!(running[0].trigger, Trigger::Schedule);
        assert_eq!(running[0].stdout, None);
        assert_eq!(running[0].exit_code, None);
        assert_eq!(running[0].ended_at, None);
        assert_eq!(running[0].duration, None);

        // finalize updates the SAME row in place.
        execs
            .finalize(
                "exec_1",
                ExecutionStatus::Success,
                Some("out"),
                Some("err"),
                Some(0),
                None,
                Some("session_42"),
                now + 500,
                500,
            )
            .await
            .unwrap();

        let after = execs.list_for_job("job_1", 10, 0).await.unwrap();
        assert_eq!(after.len(), 1, "finalize must not create a new row");
        assert_eq!(after[0].id, "exec_1");
        assert_eq!(after[0].status, ExecutionStatus::Success);
        assert_eq!(after[0].stdout.as_deref(), Some("out"));
        assert_eq!(after[0].exit_code, Some(0));
        assert_eq!(after[0].error, None);
        assert_eq!(after[0].result_ref.as_deref(), Some("session_42"));
        assert_eq!(after[0].ended_at, Some(now + 500));
        assert_eq!(after[0].duration, Some(500));

        // finalize with Running status is rejected.
        assert!(execs
            .finalize(
                "exec_1",
                ExecutionStatus::Running,
                None,
                None,
                None,
                None,
                None,
                now,
                0,
            )
            .await
            .is_err());

        // finalize a missing row is not_found.
        assert!(execs
            .finalize(
                "ghost",
                ExecutionStatus::Failed,
                None,
                None,
                None,
                None,
                None,
                now,
                0,
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_list_for_job_newest_first() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        for i in 0..3 {
            execs
                .insert_running(
                    &format!("exec_{}", i),
                    "job_1",
                    Trigger::Manual,
                    1,
                    now + i * 100,
                    now + i * 100,
                )
                .await
                .unwrap();
        }

        let listed = execs.list_for_job("job_1", 10, 0).await.unwrap();
        let ids: Vec<&str> = listed.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["exec_2", "exec_1", "exec_0"]);
    }

    /// Helper: insert `count` finalized executions for a job with ascending
    /// started_at, then return their ids in insertion order (oldest-first).
    async fn seed_finalized(
        execs: &JobExecutionRepository,
        job_id: &str,
        count: i64,
        base: Timestamp,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        for i in 0..count {
            let id = format!("{}_e{}", job_id, i);
            let started = base + i;
            execs
                .insert_running(&id, job_id, Trigger::Schedule, 1, started, started)
                .await
                .unwrap();
            execs
                .finalize(
                    &id,
                    ExecutionStatus::Success,
                    None,
                    None,
                    Some(0),
                    None,
                    None,
                    started + 1,
                    1,
                )
                .await
                .unwrap();
            ids.push(id);
        }
        ids
    }

    #[tokio::test]
    async fn test_prune_exactly_n_keeps_all() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        seed_finalized(&execs, "job_1", 100, now).await;

        let removed = execs.prune_to("job_1", 100).await.unwrap();
        assert_eq!(removed, 0, "exactly N keeps all");
        assert_eq!(execs.list_for_job("job_1", 1000, 0).await.unwrap().len(), 100);
    }

    #[tokio::test]
    async fn test_prune_n_plus_one_drops_oldest() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        let ids = seed_finalized(&execs, "job_1", 101, now).await;

        let removed = execs.prune_to("job_1", 100).await.unwrap();
        assert_eq!(removed, 1, "N+1 drops exactly the oldest");

        let remaining = execs.list_for_job("job_1", 1000, 0).await.unwrap();
        assert_eq!(remaining.len(), 100);
        // Oldest (ids[0]) must be gone; newest must remain.
        let remaining_ids: Vec<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
        assert!(!remaining_ids.contains(&ids[0].as_str()));
        assert!(remaining_ids.contains(&ids[100].as_str()));
    }

    #[tokio::test]
    async fn test_prune_is_per_job_fifo() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_a", now).await;
        seed_job(&jobs, "job_b", now).await;
        let a_ids = seed_finalized(&execs, "job_a", 5, now).await;
        seed_finalized(&execs, "job_b", 5, now).await;

        // Prune job_a to 3: drops its 2 oldest; job_b untouched.
        let removed = execs.prune_to("job_a", 3).await.unwrap();
        assert_eq!(removed, 2);

        let a_remaining = execs.list_for_job("job_a", 100, 0).await.unwrap();
        assert_eq!(a_remaining.len(), 3);
        let a_remaining_ids: Vec<&str> = a_remaining.iter().map(|e| e.id.as_str()).collect();
        // Kept the 3 newest (a_ids[2..5]); dropped a_ids[0], a_ids[1].
        assert!(!a_remaining_ids.contains(&a_ids[0].as_str()));
        assert!(!a_remaining_ids.contains(&a_ids[1].as_str()));
        assert!(a_remaining_ids.contains(&a_ids[4].as_str()));

        // job_b is fully intact.
        assert_eq!(execs.list_for_job("job_b", 100, 0).await.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn test_prune_never_deletes_running_rows() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        // 3 finalized + 2 still-running (oldest started_at).
        execs
            .insert_running("run_old_0", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        execs
            .insert_running("run_old_1", "job_1", Trigger::Schedule, 1, now + 1, now + 1)
            .await
            .unwrap();
        seed_finalized(&execs, "job_1", 3, now + 100).await;

        // Keep only 1 finalized row; running rows must survive regardless.
        let removed = execs.prune_to("job_1", 1).await.unwrap();
        assert_eq!(removed, 2, "only the 2 oldest finalized rows are pruned");

        let remaining = execs.list_for_job("job_1", 100, 0).await.unwrap();
        let remaining_ids: Vec<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
        assert!(remaining_ids.contains(&"run_old_0"));
        assert!(remaining_ids.contains(&"run_old_1"));
        // 2 running + 1 kept finalized = 3 rows.
        assert_eq!(remaining.len(), 3);
    }

    #[tokio::test]
    async fn test_reconcile_stale_running_marks_terminal() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;

        // 2 stale running rows + 1 already-finalized row.
        execs
            .insert_running("stale_0", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        execs
            .insert_running("stale_1", "job_1", Trigger::Manual, 1, now + 10, now + 10)
            .await
            .unwrap();
        execs
            .insert_running("done", "job_1", Trigger::Schedule, 1, now + 20, now + 20)
            .await
            .unwrap();
        execs
            .finalize(
                "done",
                ExecutionStatus::Success,
                None,
                None,
                Some(0),
                None,
                None,
                now + 25,
                5,
            )
            .await
            .unwrap();

        let affected = execs
            .reconcile_stale_running(ExecutionStatus::Failed, "interrupted", now + 1000)
            .await
            .unwrap();
        assert_eq!(affected, 2, "only the running rows are reconciled");

        let all = execs.list_for_job("job_1", 100, 0).await.unwrap();
        for e in &all {
            assert_ne!(e.status, ExecutionStatus::Running, "no running rows remain");
        }
        let stale: Vec<&JobExecution> =
            all.iter().filter(|e| e.id.starts_with("stale_")).collect();
        for e in stale {
            assert_eq!(e.status, ExecutionStatus::Failed);
            assert_eq!(e.error.as_deref(), Some("interrupted"));
            assert_eq!(e.ended_at, Some(now + 1000));
            assert!(e.duration.unwrap() >= 0);
        }
        // The already-finalized row keeps its success status untouched.
        let done = all.iter().find(|e| e.id == "done").unwrap();
        assert_eq!(done.status, ExecutionStatus::Success);
        assert_eq!(done.error, None);
    }

    #[tokio::test]
    async fn test_delete_job_cascades_executions_with_fk_on() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        // sqlx enables PRAGMA foreign_keys by default; assert it for this test.
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(db_arc.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(fk, 1, "FK enforcement must be ON for cascade");

        seed_job(&jobs, "job_1", now).await;
        execs
            .insert_running("exec_1", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        assert_eq!(execs.list_for_job("job_1", 10, 0).await.unwrap().len(), 1);

        jobs.delete("job_1").await.unwrap();

        // Cascade removed the execution row.
        assert_eq!(execs.list_for_job("job_1", 10, 0).await.unwrap().len(), 0);
    }
}
