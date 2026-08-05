// Business layer for Agent-mode projects (sessions grouped by working
// directory), on top of `AgentProjectRepository`.
//
// Path validation is as strict as `AgentSessionService::validate_working_dir`
// but not shared with it: a session's working_dir may be None/empty, whereas a
// project's path is its identity and must be non-empty — a shared helper would
// risk letting the empty-value case through here.

use crate::models::AppError;
use crate::services::agent_jsonl_store::{delete_session_file, session_cwd};
use crate::services::Database;
use crate::storage::types::{AgentProject, CreateAgentProjectRequest, UUID};
use crate::storage::AgentProjectRepository;
use sqlx::Row;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct AgentProjectService {
    /// Held directly to query a project's session ids (needed to abort each one
    /// before delete); neither repository exposes that query.
    db: Arc<Database>,
    repository: AgentProjectRepository,
}

impl AgentProjectService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: AgentProjectRepository::new(Arc::clone(&db)),
            db,
        }
    }

    /// Get-or-create a project keyed by canonical path.
    ///
    /// An existing project (symlink aliases included) is returned untouched.
    /// Empty / relative / file / missing paths yield `VALIDATION_ERROR` and
    /// write no row.
    pub async fn create_project(&self, path: String) -> Result<AgentProject, AppError> {
        let canonical = Self::validate_project_path(&path)?;
        let name = default_project_name(&canonical);
        self.repository
            .create_project(&CreateAgentProjectRequest {
                path: canonical,
                name,
            })
            .await
    }

    pub async fn list_projects(&self) -> Result<Vec<AgentProject>, AppError> {
        self.repository.list_projects().await
    }

    pub async fn get_project(&self, project_id: UUID) -> Result<AgentProject, AppError> {
        match self.repository.get_project_by_id(&project_id).await? {
            Some(project) => Ok(project),
            None => Err(AppError::not_found(&format!(
                "Agent project not found: {}",
                project_id
            ))),
        }
    }

    /// Rename a project, storing the trimmed name.
    ///
    /// A blank name is a `VALIDATION_ERROR` because the project name is the
    /// sidebar group header (session rename has no such constraint). A missing
    /// project passes the repository's `NOT_FOUND` through.
    pub async fn rename_project(
        &self,
        project_id: UUID,
        name: String,
    ) -> Result<AgentProject, AppError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation_error(
                "Agent project name must not be blank",
            ));
        }
        self.repository.rename_project(&project_id, trimmed).await?;
        self.get_project(project_id).await
    }

    /// Delete a project: abort every session's run, best-effort delete each
    /// session's `<id>.jsonl`, then cascade messages / sessions / project in a
    /// single repository transaction. A missing project yields `NOT_FOUND`.
    ///
    /// `app_data_dir` is both the JSONL base and the cwd fallback for sessions
    /// without a working_dir; it must match the write side (`config_from_rows`
    /// / `session_cwd`) or the wrong directory gets cleaned.
    pub async fn delete_project(
        &self,
        project_id: UUID,
        app_data_dir: &Path,
    ) -> Result<(), AppError> {
        self.delete_project_with_abort(project_id, app_data_dir, |session_id| async move {
            crate::services::coding_agent_runtime::abort_run(&session_id);
        })
        .await
    }

    /// Body of `delete_project` with abort injected as a closure, so tests can
    /// assert the abort-then-delete ordering without starting a real run (the
    /// run registry is process-private).
    async fn delete_project_with_abort<F, Fut>(
        &self,
        project_id: UUID,
        app_data_dir: &Path,
        abort: F,
    ) -> Result<(), AppError>
    where
        F: Fn(UUID) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let sessions = self.list_sessions_for_delete(&project_id).await?;
        for (session_id, _working_dir) in &sessions {
            abort(session_id.clone()).await;
        }

        // Best-effort: a failed file delete is logged, never fatal. The SQLite
        // cascade below is what actually removes the sessions; deleting here
        // just avoids leaving orphan `<id>.jsonl` files behind.
        for (session_id, working_dir) in &sessions {
            let cwd = session_cwd(working_dir.as_deref(), app_data_dir);
            if let Err(e) = delete_session_file(app_data_dir, &cwd, session_id) {
                tracing::warn!(
                    session_id = %session_id,
                    "failed to delete JSONL transcript file on project delete, \
                     continuing with the SQLite cascade: {e}"
                );
            }
        }

        self.repository.delete_project(&project_id).await
    }

    /// `(id, working_dir)` of every session in a project; working_dir locates
    /// each session's JSONL file. A missing project yields an empty set — the
    /// repository delete that follows reports the `NOT_FOUND`.
    async fn list_sessions_for_delete(
        &self,
        project_id: &UUID,
    ) -> Result<Vec<(UUID, Option<String>)>, AppError> {
        let rows = sqlx::query("SELECT id, working_dir FROM agent_sessions WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list project sessions: {}", e))
            })?;

        rows.into_iter()
            .map(|row| {
                let id = row.try_get::<String, _>("id")?;
                // working_dir is nullable and must be decoded as Option, or
                // sqlx-sqlite silently turns a NULL TEXT into Some("").
                let working_dir = row.try_get::<Option<String>, _>("working_dir")?;
                Ok((id, working_dir))
            })
            .collect()
    }

    /// Validate and canonicalize a project path. Empty, relative, missing, or
    /// non-directory paths are rejected; otherwise the canonical absolute path
    /// is returned.
    fn validate_project_path(raw: &str) -> Result<String, AppError> {
        if raw.is_empty() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                "project path must not be empty",
                "请提供一个已存在目录的绝对路径",
            ));
        }

        let path = std::path::Path::new(raw);

        // Relative paths are rejected even when they resolve against cwd, so the
        // stored identity stays deterministic.
        if !path.is_absolute() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path must be an absolute path: {}", raw),
                "请提供一个已存在目录的绝对路径",
            ));
        }

        // canonicalize resolves symlinks and requires existence; a failure here
        // means the path is not there.
        let canonical = std::fs::canonicalize(path).map_err(|_| {
            AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path does not exist: {}", raw),
                "请提供一个已存在目录的绝对路径",
            )
        })?;

        if !canonical.is_dir() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path is not a directory: {}", raw),
                "project path 必须指向一个目录而非文件",
            ));
        }

        Ok(canonical.to_string_lossy().into_owned())
    }
}

/// Default project name: the canonical path's basename, falling back to the
/// full path when the basename is empty (e.g. root `/`).
///
/// A free `pub fn` so the SQLite/JSONL migration can derive a JSONL session's
/// project group name from its canonicalized `header.cwd` with the exact
/// algorithm `create_project` uses for `agent_projects.name`; that keeps a
/// session in the same project group across both transcript sources.
pub fn default_project_name(canonical: &str) -> String {
    std::path::Path::new(canonical)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| canonical.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    /// Test database; the returned TempDir must outlive it.
    async fn create_test_database() -> (Arc<Database>, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        );
        (db, temp_dir)
    }

    async fn count_rows(db: &Database, table: &str) -> i64 {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) AS count FROM {}",
            table
        )))
            .fetch_one(db.pool())
            .await
            .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    async fn insert_session(db: &Database, project_id: Option<&str>, name: &str) -> String {
        insert_session_with_dir(db, project_id, name, None).await
    }

    /// Like `insert_session`, but with an explicit `working_dir`.
    async fn insert_session_with_dir(
        db: &Database,
        project_id: Option<&str>,
        name: &str,
        working_dir: Option<&str>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, name, project_id, working_dir, message_count, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 0, $5, $6)
        "#,
        )
        .bind(&id)
        .bind(name)
        .bind(project_id)
        .bind(working_dir)
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();
        id
    }

    async fn insert_message(db: &Database, session_id: &str, seq: i64) {
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, 'user', '{"text":"hi"}', $4)
        "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(seq)
        .bind(now_ms())
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn create_project_rejects_empty_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let err = service
            .create_project(String::new())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_relative_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        // A relative path that may well exist relative to cwd, yet must be rejected.
        let err = service
            .create_project("src".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_file_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        // An existing FILE (not a dir).
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("a-file.txt");
        std::fs::write(&file_path, b"hello").unwrap();

        let err = service
            .create_project(file_path.to_string_lossy().into_owned())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_missing_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let err = service
            .create_project("/this/path/should/not/exist/handbox-agent-project".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_stores_canonical_path_and_basename_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let sub_dir = work_dir.path().join("alpha");
        std::fs::create_dir(&sub_dir).unwrap();
        let expected_canonical = std::fs::canonicalize(&sub_dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let created = service
            .create_project(sub_dir.to_string_lossy().into_owned())
            .await
            .expect("create failed");
        assert_eq!(created.path, expected_canonical);
        assert_eq!(created.name, "alpha");
    }

    #[tokio::test]
    async fn create_project_twice_same_dir_returns_existing_single_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let work_dir = TempDir::new().unwrap();
        let raw = work_dir.path().to_string_lossy().into_owned();

        let first = service.create_project(raw.clone()).await.unwrap();

        // Rename so a second create proves "returns existing unchanged".
        let renamed = service
            .rename_project(first.id.clone(), "Custom Name".to_string())
            .await
            .unwrap();

        let second = service.create_project(raw).await.unwrap();
        assert_eq!(second.id, first.id);
        assert_eq!(second.name, "Custom Name");
        assert_eq!(second.created_at, first.created_at);
        assert_eq!(second.updated_at, renamed.updated_at);
        assert_eq!(count_rows(&db, "agent_projects").await, 1);
    }

    #[tokio::test]
    async fn create_project_via_symlink_alias_returns_existing_single_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        // Real target dir + a symlink pointing at it.
        let target = TempDir::new().unwrap();
        let link_parent = TempDir::new().unwrap();
        let link = link_parent.path().join("link-to-dir");

        #[cfg(unix)]
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        #[cfg(not(unix))]
        return; // symlink semantics differ; covered on unix CI

        let canonical_target = std::fs::canonicalize(target.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let direct = service
            .create_project(target.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(direct.path, canonical_target);

        // The symlink alias resolves to the same canonical path -> same project.
        let via_link = service
            .create_project(link.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(via_link.id, direct.id);
        assert_eq!(via_link.path, canonical_target);
        assert_eq!(via_link.name, direct.name);
        assert_eq!(count_rows(&db, "agent_projects").await, 1);
    }

    #[tokio::test]
    async fn create_project_root_path_falls_back_to_full_path_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        // Root path: basename is empty -> name falls back to the full path.
        let created = service.create_project("/".to_string()).await.unwrap();
        assert_eq!(created.path, "/");
        assert_eq!(created.name, "/");
    }

    #[tokio::test]
    async fn list_and_get_project_roundtrip() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let created = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        let listed = service.list_projects().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let got = service.get_project(created.id.clone()).await.unwrap();
        assert_eq!(got.path, created.path);

        let err = service
            .get_project("nonexistent".to_string())
            .await
            .expect_err("expected error");
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn rename_project_rejects_blank_and_trims_valid_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let created = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        // Blank (whitespace-only) name -> VALIDATION_ERROR, nothing changed.
        let err = service
            .rename_project(created.id.clone(), "   ".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        let unchanged = service.get_project(created.id.clone()).await.unwrap();
        assert_eq!(unchanged.name, created.name);
        assert_eq!(unchanged.updated_at, created.updated_at);

        // Valid name is trimmed before storage.
        let renamed = service
            .rename_project(created.id.clone(), "  New Name  ".to_string())
            .await
            .unwrap();
        assert_eq!(renamed.name, "New Name");

        // Missing id -> NOT_FOUND passthrough.
        let err = service
            .rename_project("missing".to_string(), "x".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// Seed `<id>.jsonl` for a session under `base_dir` keyed by its working_dir,
    /// returning the file path so the test can assert on its (later) absence.
    fn seed_session_jsonl(
        base_dir: &std::path::Path,
        working_dir: Option<&str>,
        session_id: &str,
    ) -> std::path::PathBuf {
        let cwd = session_cwd(working_dir, base_dir);
        let path =
            crate::services::agent_jsonl_store::ensure_session_file(base_dir, &cwd, session_id, 1)
                .expect("seed jsonl");
        assert!(path.exists(), "precondition: seeded JSONL exists");
        path
    }

    #[tokio::test]
    async fn delete_project_aborts_each_session_before_cascade() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());
        let base = TempDir::new().unwrap();

        let work_dir = TempDir::new().unwrap();
        let work_dir_str = work_dir.path().to_string_lossy().into_owned();
        let project = service.create_project(work_dir_str.clone()).await.unwrap();

        let s1 = insert_session(&db, Some(&project.id), "s1").await;
        let s2 = insert_session(&db, Some(&project.id), "s2").await;
        insert_message(&db, &s1, 0).await;
        insert_message(&db, &s2, 0).await;
        // A bystander session outside the project must NOT be aborted.
        let outsider = insert_session(&db, None, "outsider").await;

        // Recording abort: capture (session_id, rows still present at abort
        // time) to prove abort happens BEFORE the cascade delete.
        let abort_log: Arc<Mutex<Vec<(String, i64)>>> = Arc::new(Mutex::new(Vec::new()));
        let db_for_abort = db.clone();
        let log_for_abort = Arc::clone(&abort_log);
        let abort = move |session_id: String| {
            let db = db_for_abort.clone();
            let log = Arc::clone(&log_for_abort);
            async move {
                let still_present: i64 =
                    sqlx::query("SELECT COUNT(*) AS count FROM agent_sessions WHERE id = $1")
                        .bind(&session_id)
                        .fetch_one(db.pool())
                        .await
                        .unwrap()
                        .try_get("count")
                        .unwrap();
                log.lock().await.push((session_id, still_present));
            }
        };

        service
            .delete_project_with_abort(project.id.clone(), base.path(), abort)
            .await
            .unwrap();

        // Both project sessions were aborted while their rows still existed.
        let log = abort_log.lock().await;
        assert_eq!(log.len(), 2);
        let aborted: Vec<&str> = log.iter().map(|(id, _)| id.as_str()).collect();
        assert!(aborted.contains(&s1.as_str()));
        assert!(aborted.contains(&s2.as_str()));
        assert!(!aborted.contains(&outsider.as_str()));
        assert!(log.iter().all(|(_, present)| *present == 1));

        // Cascade: project, its sessions and their messages are gone.
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_session_messages").await, 0);
        // Only the ungrouped bystander session remains.
        assert_eq!(count_rows(&db, "agent_sessions").await, 1);
    }

    /// Deleting a project removes its sessions from SQLite and their JSONL
    /// files from disk, including a session with no `working_dir` (rooted at
    /// the app-data dir). A session outside the project keeps both.
    #[tokio::test]
    async fn delete_project_cascades_jsonl_files_for_every_session() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());
        let base = TempDir::new().unwrap();

        let work_dir = TempDir::new().unwrap();
        let work_dir_str = work_dir.path().to_string_lossy().into_owned();
        let project = service.create_project(work_dir_str.clone()).await.unwrap();

        // Two in-project sessions: one with a working_dir, one without (so the
        // app-data-dir cwd fallback is exercised). Seed each one's JSONL file.
        let s1 = insert_session_with_dir(&db, Some(&project.id), "s1", Some(&work_dir_str)).await;
        let s2 = insert_session_with_dir(&db, Some(&project.id), "s2", None).await;
        let s1_jsonl = seed_session_jsonl(base.path(), Some(&work_dir_str), &s1);
        let s2_jsonl = seed_session_jsonl(base.path(), None, &s2);

        // A bystander session OUTSIDE the project, with its own JSONL file.
        let outsider = insert_session_with_dir(&db, None, "outsider", Some(&work_dir_str)).await;
        let outsider_jsonl = seed_session_jsonl(base.path(), Some(&work_dir_str), &outsider);

        service
            .delete_project(project.id.clone(), base.path())
            .await
            .unwrap();

        // SQLite: project + its sessions gone; only the bystander remains.
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 1);

        // Disk: every in-project session's JSONL file is removed.
        assert!(
            !s1_jsonl.exists(),
            "in-project session JSONL must be deleted"
        );
        assert!(
            !s2_jsonl.exists(),
            "no-working-dir session JSONL must be deleted"
        );
        // The bystander's JSONL survives untouched.
        assert!(
            outsider_jsonl.exists(),
            "a session outside the project keeps its JSONL file"
        );
    }

    #[tokio::test]
    async fn delete_project_public_path_cascades_and_passes_through_not_found() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());
        // The public `delete_project` drives the real `abort_run`; on sessions
        // without an active run that is a clean no-op, so the full public path
        // is exercised end to end.
        let base = TempDir::new().unwrap();

        let work_dir = TempDir::new().unwrap();
        let project = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let session = insert_session(&db, Some(&project.id), "s1").await;
        insert_message(&db, &session, 0).await;

        service
            .delete_project(project.id.clone(), base.path())
            .await
            .unwrap();
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
        assert_eq!(count_rows(&db, "agent_session_messages").await, 0);

        // Missing id -> NOT_FOUND passthrough (and no panic from abort phase).
        let err = service
            .delete_project("missing".to_string(), base.path())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// A JSONL file delete failure for one session is best-effort: it is logged
    /// and the SQLite cascade still removes the project. Modelled here by a
    /// session whose JSONL file simply does not exist (delete is a clean no-op),
    /// proving the cascade does not depend on every file being present.
    #[tokio::test]
    async fn delete_project_tolerates_missing_jsonl_file() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());
        let base = TempDir::new().unwrap();

        let work_dir = TempDir::new().unwrap();
        let project = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        // Session with no JSONL file at all.
        let _session = insert_session(&db, Some(&project.id), "s1").await;

        service
            .delete_project(project.id.clone(), base.path())
            .await
            .expect("a missing JSONL file must not block the SQLite cascade");
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }
}
