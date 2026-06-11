//! Skill discovery IPC command.
//!
//! Exposes [`skill_list`], a read-only view over [`SkillService::discover`] for
//! the settings UI and the agent-input skill toggle. The command never fails on
//! a malformed skill: discovery already returns `(skills, errors)`, and this
//! layer folds *both* into a flat [`SkillInfo`] list — clean skills carry their
//! metadata with an empty `diagnostics`, while validation failures surface as
//! entries with `description`/`body` cleared and a non-empty `diagnostics`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::models::AppError;
use crate::services::skills::{Skill, SkillError, SourceScope};
use crate::services::SkillService;

/// A single discovered skill as seen by the frontend.
///
/// This is a read-only discovery object (no DB id); `name` is the key. A
/// successfully validated skill has `description`/`body` set and an empty
/// `diagnostics`. A skill that failed validation has `description`/`body` as
/// `None` and one or more diagnostic strings.
///
/// `path` points at the skill *directory* (the `SKILL.md` parent), never at the
/// `SKILL.md` file itself.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    /// Canonical skill name. For failed skills this is best-effort, derived
    /// from the offending path's parent directory name.
    pub name: String,
    /// Frontmatter description; `None` for a skill that failed validation.
    pub description: Option<String>,
    /// Discovery scope. Serializes to `"project"` / `"user"` / `"appData"`.
    pub scope: SourceScope,
    /// The skill directory (parent of `SKILL.md`).
    pub path: PathBuf,
    /// `SKILL.md` body; `None` for a skill that failed validation.
    pub body: Option<String>,
    /// Validation diagnostics. Empty for a clean skill; non-empty otherwise.
    pub diagnostics: Vec<String>,
}

/// List discovered skills (including validation diagnostics) for the given
/// working directory.
///
/// `working_dir` (wired from the camelCase `workingDir` argument by Tauri)
/// scopes the project-level skill root; when absent, only the user and
/// app-data scopes are searched. A non-existent or otherwise unusable project
/// directory is non-fatal — discovery silently skips it and still returns the
/// user/app-data skills.
///
/// The command itself does not fail on a malformed skill; per-skill problems
/// are reported as `diagnostics` on the offending [`SkillInfo`]. It returns an
/// `AppError` only on a command-level fault.
#[tauri::command]
pub async fn skill_list(
    working_dir: Option<String>,
    skill_service: State<'_, Arc<SkillService>>,
) -> Result<Vec<SkillInfo>, AppError> {
    let (skills, errors) = skill_service.discover(working_dir.as_deref().map(Path::new));
    Ok(to_skill_infos(skills, errors))
}

/// Fold discovery output `(skills, errors)` into a flat [`SkillInfo`] list.
///
/// Pure and filesystem-independent so it can be unit-tested directly. Clean
/// skills map to entries with an empty `diagnostics`; each [`SkillError`] maps
/// to a diagnostic entry whose `name`/`scope`/`path` are derived from the error
/// (which carries the offending path for exactly this purpose).
fn to_skill_infos(skills: Vec<Skill>, errors: Vec<SkillError>) -> Vec<SkillInfo> {
    let mut infos: Vec<SkillInfo> = Vec::with_capacity(skills.len() + errors.len());

    for skill in skills {
        infos.push(SkillInfo {
            name: skill.name,
            description: Some(skill.description),
            scope: skill.source.scope,
            path: skill_dir(&skill.source.path),
            body: Some(skill.body),
            diagnostics: Vec::new(),
        });
    }

    for error in errors {
        infos.push(error_to_skill_info(error));
    }

    infos
}

/// Map a [`SkillError`] to a diagnostic-only [`SkillInfo`].
///
/// Exhaustive over all six variants so a new variant forces a compile error
/// here. Each variant carries the offending path used to derive the skill
/// directory and a best-effort name; the scope is unknown at the error site so
/// it defaults to the lowest-priority [`SourceScope::AppData`].
fn error_to_skill_info(error: SkillError) -> SkillInfo {
    let diagnostic = error.to_string();
    let path = match &error {
        SkillError::Io { path, .. }
        | SkillError::Loader { path, .. }
        | SkillError::MissingDescription { path }
        | SkillError::DescriptionTooLong { path, .. }
        | SkillError::NameMismatch { path, .. }
        | SkillError::InvalidName { path, .. } => path.clone(),
    };

    let dir = skill_dir(&path);
    SkillInfo {
        name: dir_name(&dir),
        description: None,
        scope: SourceScope::AppData,
        path: dir,
        body: None,
        diagnostics: vec![diagnostic],
    }
}

/// The skill directory for a source path: if `path` names a `SKILL.md` file,
/// return its parent; otherwise return `path` unchanged (it is already the
/// directory, or its shape is unexpected — leave it intact rather than guess).
fn skill_dir(path: &Path) -> PathBuf {
    if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

/// Best-effort skill name from a directory path: its basename, or an empty
/// string when the path has no usable final component.
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// Write `<root>/<dir>/SKILL.md` with the given raw content, creating
    /// parents. Returns the skill directory.
    fn write_skill_raw(root: &Path, dir: &str, content: &str) -> PathBuf {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
        skill_dir
    }

    /// Minimal valid `SKILL.md` content (frontmatter description + body).
    fn skill_md(description: &str, body: &str) -> String {
        format!("---\ndescription: {description}\n---\n{body}")
    }

    /// Index a result list by name for order-independent assertions.
    fn by_name(infos: &[SkillInfo]) -> HashMap<&str, &SkillInfo> {
        infos.iter().map(|i| (i.name.as_str(), i)).collect()
    }

    /// Run `skill_list`'s inner logic against fixture roots — exercises the
    /// real `SkillService::discover` plus the `to_skill_infos` fold, which is
    /// everything `skill_list` does apart from the Tauri `State` unwrap.
    fn run(app: &Path, user: &Path, working_dir: Option<&Path>) -> Vec<SkillInfo> {
        let svc = SkillService::for_test(app.to_path_buf(), user.to_path_buf());
        let (skills, errors) = svc.discover(working_dir);
        to_skill_infos(skills, errors)
    }

    // VAL-IPC-001: no workingDir → user + app-data skills, each with
    // name/description/scope/path/body present and empty diagnostics.
    #[test]
    fn val_ipc_001_lists_user_and_appdata_with_full_fields() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("from app", "a body"));
        write_skill_raw(user.path(), "beta", &skill_md("from user", "b body"));

        let infos = run(app.path(), user.path(), None);
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 2, "expected two skills: {infos:?}");

        let alpha = idx["alpha"];
        assert_eq!(alpha.scope, SourceScope::AppData);
        assert_eq!(alpha.description.as_deref(), Some("from app"));
        assert_eq!(alpha.body.as_deref(), Some("a body"));
        assert!(alpha.diagnostics.is_empty());
        assert_eq!(alpha.path, app.path().join("alpha"));

        let beta = idx["beta"];
        assert_eq!(beta.scope, SourceScope::User);
        assert_eq!(beta.description.as_deref(), Some("from user"));
        assert_eq!(beta.body.as_deref(), Some("b body"));
        assert!(beta.diagnostics.is_empty());
    }

    // VAL-IPC-002: a working dir brings the project scope into the result.
    #[test]
    fn val_ipc_002_working_dir_includes_project_scope() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let proj_skills = proj.path().join(".handbox").join("skills");
        write_skill_raw(app.path(), "alpha", &skill_md("from app", "a"));
        write_skill_raw(&proj_skills, "gamma", &skill_md("from proj", "g"));

        let infos = run(app.path(), user.path(), Some(proj.path()));
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 2, "expected app + project skills: {infos:?}");
        assert_eq!(idx["gamma"].scope, SourceScope::Project);
        assert_eq!(idx["gamma"].path, proj_skills.join("gamma"));
    }

    // VAL-IPC-003: all three scopes empty → an empty list (success).
    #[test]
    fn val_ipc_003_all_scopes_empty_returns_empty() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let infos = run(app.path(), user.path(), Some(proj.path()));
        assert!(infos.is_empty(), "expected empty result: {infos:?}");
    }

    // VAL-IPC-004: a validation-failed skill appears with diagnostics, and the
    // command (the fold) does not drop or fail on it. Valid siblings still
    // surface with empty diagnostics.
    #[test]
    fn val_ipc_004_failed_skill_surfaces_with_diagnostics() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        // Valid skill in user scope.
        write_skill_raw(user.path(), "good", &skill_md("ok", "body"));
        // Invalid skill: missing the required `description`.
        write_skill_raw(app.path(), "no-desc", "---\nname: no-desc\n---\nbody");

        let infos = run(app.path(), user.path(), None);
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 2, "good + failed expected: {infos:?}");

        let good = idx["good"];
        assert!(good.diagnostics.is_empty());
        assert!(good.description.is_some());

        let bad = idx["no-desc"];
        assert!(bad.description.is_none(), "failed skill has no description");
        assert!(bad.body.is_none(), "failed skill has no body");
        assert_eq!(bad.diagnostics.len(), 1, "one diagnostic: {bad:?}");
        assert!(
            bad.diagnostics[0].contains("description"),
            "diagnostic should name the problem: {:?}",
            bad.diagnostics
        );
        // path points at the skill directory derived from the error path.
        assert_eq!(bad.path, app.path().join("no-desc"));
    }

    // VAL-IPC-005: a same-named skill across scopes collapses to the shadow
    // winner only (highest scope), never duplicated.
    #[test]
    fn val_ipc_005_same_name_shows_only_shadow_winner() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let proj_skills = proj.path().join(".handbox").join("skills");
        write_skill_raw(app.path(), "shared", &skill_md("from app", "app body"));
        write_skill_raw(user.path(), "shared", &skill_md("from user", "user body"));
        write_skill_raw(&proj_skills, "shared", &skill_md("from proj", "proj body"));

        let infos = run(app.path(), user.path(), Some(proj.path()));
        assert_eq!(infos.len(), 1, "dedup to one winner: {infos:?}");
        assert_eq!(infos[0].name, "shared");
        assert_eq!(infos[0].scope, SourceScope::Project);
        assert_eq!(infos[0].description.as_deref(), Some("from proj"));
        assert_eq!(infos[0].body.as_deref(), Some("proj body"));
    }

    // VAL-IPC-006: SkillInfo.path is the skill DIRECTORY, not the SKILL.md file.
    #[test]
    fn val_ipc_006_path_points_at_skill_directory() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let dir = write_skill_raw(app.path(), "alpha", &skill_md("d", "b"));

        let infos = run(app.path(), user.path(), None);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].path, dir);
        assert_ne!(
            infos[0].path.file_name().and_then(|n| n.to_str()),
            Some("SKILL.md"),
            "path must not be the SKILL.md file"
        );
    }

    // VAL-IPC-007: scope serializes to the camelCase literals the wire expects.
    #[test]
    fn val_ipc_007_scope_serializes_to_literals() {
        let info = SkillInfo {
            name: "x".to_string(),
            description: Some("d".to_string()),
            scope: SourceScope::AppData,
            path: PathBuf::from("/skills/x"),
            body: Some("b".to_string()),
            diagnostics: Vec::new(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["scope"], "appData");

        for (scope, literal) in [
            (SourceScope::Project, "project"),
            (SourceScope::User, "user"),
            (SourceScope::AppData, "appData"),
        ] {
            let v = serde_json::to_value(SkillInfo {
                scope,
                ..info.clone()
            })
            .unwrap();
            assert_eq!(v["scope"], literal, "scope literal for {scope:?}");
        }

        // camelCase field renaming on the struct as a whole.
        assert!(json.get("description").is_some());
        assert!(json.get("diagnostics").is_some());
    }

    // VAL-IPC-008: a non-existent / relative project working dir is non-fatal —
    // the project scope is silently skipped and user/app-data skills remain.
    #[test]
    fn val_ipc_008_bad_working_dir_is_non_fatal() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(user.path(), "beta", &skill_md("from user", "b"));

        // Non-existent absolute working dir.
        let missing = app.path().join("does-not-exist-project");
        let infos = run(app.path(), user.path(), Some(&missing));
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 1, "only user skill survives: {infos:?}");
        assert_eq!(idx["beta"].scope, SourceScope::User);

        // Relative working dir (no such directory under CWD) — still non-fatal.
        let relative = Path::new("definitely-not-a-real-relative-dir");
        let infos2 = run(app.path(), user.path(), Some(relative));
        assert_eq!(by_name(&infos2).len(), 1, "user skill survives: {infos2:?}");
    }

    // VAL-IPC-009: command-level fault → structured AppError. Discovery is
    // lenient (per-skill errors become diagnostics, not command failures), so a
    // command-level `Err(AppError)` branch is unreachable from `to_skill_infos`.
    // This pins the only command-level fault surface: an `AppError` round-trips
    // through serde as a structured `{ code, message, hint }`, which is the
    // contract the frontend relies on. Documented as hard-to-reach by design.
    #[test]
    fn val_ipc_009_app_error_is_structured() {
        let err = AppError::internal_error("skill discovery failed");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "INTERNAL_ERROR");
        assert!(json["message"].is_string());
        assert!(json.get("hint").is_some());
    }

    // Exhaustiveness sanity: each of the six SkillError variants folds into a
    // diagnostic SkillInfo (description/body cleared, one diagnostic, name and
    // path derived from the error path).
    #[test]
    fn every_skill_error_variant_maps_to_diagnostic_info() {
        let p = || PathBuf::from("/skills/widget/SKILL.md");
        let errors = vec![
            SkillError::Io {
                path: p(),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            },
            SkillError::Loader {
                path: p(),
                source: crate::utils::frontmatter::FrontmatterError::UnterminatedFrontmatter,
            },
            SkillError::MissingDescription { path: p() },
            SkillError::DescriptionTooLong {
                path: p(),
                actual: 2000,
                max: 1024,
            },
            SkillError::NameMismatch {
                path: p(),
                frontmatter_name: "other".to_string(),
                dir_name: "widget".to_string(),
            },
            SkillError::InvalidName {
                path: p(),
                name: "Bad_Name".to_string(),
                reason: "invalid characters".to_string(),
            },
        ];

        let infos = to_skill_infos(Vec::new(), errors);
        assert_eq!(infos.len(), 6);
        for info in &infos {
            assert_eq!(info.name, "widget", "name from parent dir: {info:?}");
            assert_eq!(info.path, PathBuf::from("/skills/widget"));
            assert!(info.description.is_none());
            assert!(info.body.is_none());
            assert_eq!(info.diagnostics.len(), 1);
            assert!(!info.diagnostics[0].is_empty());
        }
    }
}
