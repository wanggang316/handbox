//! Skill discovery and file-reading IPC commands.
//!
//! Exposes [`skill_list`], a read-only view over [`SkillService::discover`] for
//! the settings UI and the agent-input skill toggle. The command never fails on
//! a malformed skill: discovery already returns `(skills, errors)`, and this
//! layer folds *both* into a flat [`SkillInfo`] list — clean skills carry their
//! metadata with an empty `diagnostics`, while validation failures surface as
//! entries with `description`/`body` cleared and a non-empty `diagnostics`.
//!
//! [`skill_files`] and [`skill_file_read`] back the detail view, which shows a
//! skill as the directory it really is rather than only its `SKILL.md`. Both
//! address a skill by *name* and re-resolve the directory through discovery, so
//! no caller-supplied path reaches the filesystem as a root — that, plus
//! [`resolve_skill_file`]'s containment check, is what keeps them from being a
//! general-purpose file reader.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::models::AppError;
use crate::services::skills::{Skill, SkillError, SourceScope};
use crate::services::{SettingsService, SkillService};

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
    /// Whether the skill name appears in the global `skills.disabled` list
    /// (exact-string match, applied after cross-scope dedup).
    pub disabled: bool,
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
/// `AppError` only on a command-level fault — notably an unreadable or
/// structurally invalid `config.json`, which surfaces as the settings
/// service's structured `INTERNAL_ERROR` (the file is left untouched).
#[tauri::command]
pub async fn skill_list(
    working_dir: Option<String>,
    skill_service: State<'_, Arc<SkillService>>,
    settings_service: State<'_, SettingsService>,
) -> Result<Vec<SkillInfo>, AppError> {
    let settings = settings_service.get_settings()?;
    let (skills, errors) = skill_service.discover(working_dir.as_deref().map(Path::new));
    Ok(to_skill_infos(skills, errors, &settings.skills.disabled))
}

/// Maximum accepted skill-name length in bytes (after trimming). Generous
/// versus real skill names (lowercase dir basenames) but bounds what an
/// arbitrary IPC caller can grow the persisted disabled list with.
const MAX_SKILL_NAME_BYTES: usize = 256;

/// Validate a caller-supplied skill name at the IPC boundary and return the
/// trimmed form that gets stored.
///
/// Rejected with a structured `VALIDATION_ERROR`: empty or whitespace-only
/// names, names longer than [`MAX_SKILL_NAME_BYTES`] bytes after trimming,
/// and names containing control characters. Trimming does not change
/// exact-match semantics — discovery-produced names carry no surrounding
/// whitespace.
fn validate_skill_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_error("技能名称不能为空"));
    }
    if trimmed.len() > MAX_SKILL_NAME_BYTES {
        return Err(AppError::validation_error(&format!(
            "技能名称过长（最多 {MAX_SKILL_NAME_BYTES} 字节）"
        )));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::validation_error("技能名称不能包含控制字符"));
    }
    Ok(trimmed)
}

/// Set a skill's global disabled flag in the config.json `skills.disabled`
/// list.
///
/// The name is validated first ([`validate_skill_name`]): empty/whitespace,
/// oversized or control-character names are rejected with a structured
/// `VALIDATION_ERROR` before any settings I/O, and the trimmed form is what
/// gets stored. Then a thin wrapper over
/// [`SettingsService::set_skill_disabled`], which performs a whole-file
/// read-modify-write: `disabled = true` inserts `name` (dedup),
/// `disabled = false` removes every equal entry, and all other settings
/// sections are preserved. A read, parse or disk-write failure surfaces as the
/// settings service's structured `{ code, message, hint }` AppError — never a
/// panic. Callers re-fetch `skill_list` to observe the updated flags.
#[tauri::command]
pub async fn skill_set_disabled(
    name: String,
    disabled: bool,
    settings_service: State<'_, SettingsService>,
) -> Result<(), AppError> {
    let name = validate_skill_name(&name)?;
    settings_service.set_skill_disabled(name, disabled)?;
    Ok(())
}

/// Fold discovery output `(skills, errors)` into a flat [`SkillInfo`] list,
/// annotating each entry's `disabled` flag from the global disabled list.
///
/// Pure and filesystem-independent so it can be unit-tested directly. Clean
/// skills map to entries with an empty `diagnostics`; each [`SkillError`] maps
/// to a diagnostic entry whose `name`/`scope`/`path` are derived from the error
/// (which carries the offending path for exactly this purpose).
///
/// The `disabled` annotation is an exact-string name match applied after
/// discovery's cross-scope dedup, so a shadowed name yields exactly one
/// (winning-scope) row. The list itself is opaque storage: orphan, duplicate,
/// empty or case-mismatched entries are simply inert here — no phantom rows,
/// no false disables, no errors.
fn to_skill_infos(
    skills: Vec<Skill>,
    errors: Vec<SkillError>,
    disabled: &[String],
) -> Vec<SkillInfo> {
    let mut infos: Vec<SkillInfo> = Vec::with_capacity(skills.len() + errors.len());

    for skill in skills {
        infos.push(SkillInfo {
            name: skill.name,
            description: Some(skill.description),
            scope: skill.source.scope,
            path: skill_dir(&skill.source.path),
            body: Some(skill.body),
            diagnostics: Vec::new(),
            disabled: false,
        });
    }

    for error in errors {
        infos.push(error_to_skill_info(error));
    }

    for info in &mut infos {
        info.disabled = disabled.iter().any(|name| name == &info.name);
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
        disabled: false,
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

/// One file inside a skill's directory, as offered by the detail view.
///
/// `rel_path` is the identity: it is what [`skill_file_read`] takes back, and
/// it is always `/`-separated so the frontend can use it verbatim as a key and
/// a label regardless of platform.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillFile {
    /// Path relative to the skill directory, `/`-separated.
    pub rel_path: String,
    pub size: u64,
    /// Whether [`skill_file_read`] will hand back this file's text. Binary
    /// assets and oversized files are still listed — a skill's shape is worth
    /// seeing even where its bytes are not renderable.
    pub readable: bool,
}

/// Caps on what a skill directory listing will walk and what a single file may
/// weigh. A skill is prose plus a handful of scripts; these bound the damage a
/// pathological directory (a checked-in `node_modules`, a video) can do to the
/// IPC payload and the renderer.
const MAX_SKILL_FILES: usize = 200;
const MAX_SKILL_FILE_DEPTH: usize = 4;
const MAX_SKILL_FILE_BYTES: u64 = 512 * 1024;

/// Directory names never worth listing: build output and VCS bookkeeping that
/// no skill author means to publish as part of the skill.
const SKIPPED_DIRS: &[&str] = &["node_modules", "__pycache__", "target", "dist"];

/// Extensions the detail view cannot render. Everything else is treated as
/// text: a skill directory holds prose, scripts and templates, so naming the
/// few binary asset types hides less than allow-listing the text ones would.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "icns", "svgz", "pdf", "zip", "gz", "tar",
    "bz2", "xz", "7z", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "mov", "wav", "avi",
    "webm", "so", "dylib", "dll", "exe", "bin", "wasm", "db", "sqlite",
];

/// Whether a listed path is a binary asset, decided by extension alone.
///
/// Pure so the classification is testable without a filesystem. Extension
/// matching is ASCII-case-insensitive; a file with no extension counts as text
/// (skills ship `Makefile`, `LICENSE` and bare scripts).
fn is_binary_path(rel_path: &str) -> bool {
    rel_path
        .rsplit_once('.')
        .map(|(_, ext)| {
            BINARY_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

/// Validate a caller-supplied relative path and return it as a `PathBuf`.
///
/// Only plain forward-slash-separated components are accepted: anything
/// absolute, any `.` or `..` segment, any backslash and any control character
/// is rejected with a structured `VALIDATION_ERROR`. This is the first of two
/// defences — [`resolve_skill_file`] still confirms the resolved path stayed
/// inside the skill directory, which is what catches escapes through symlinks.
fn sanitize_rel_path(rel_path: &str) -> Result<PathBuf, AppError> {
    if rel_path.is_empty() {
        return Err(AppError::validation_error("文件路径不能为空"));
    }
    if rel_path.contains('\\') || rel_path.chars().any(char::is_control) {
        return Err(AppError::validation_error("文件路径包含非法字符"));
    }

    let mut out = PathBuf::new();
    for segment in rel_path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(AppError::validation_error(
                "文件路径必须是技能目录下的相对路径",
            ));
        }
        out.push(segment);
    }
    Ok(out)
}

/// Resolve `rel_path` against a skill directory, refusing anything that lands
/// outside it.
///
/// Both sides are canonicalized before the containment check so a symlink
/// inside the skill directory cannot be used to read arbitrary files: the
/// symlink resolves to its target, and a target outside the (also resolved)
/// skill directory fails the prefix test.
fn resolve_skill_file(dir: &Path, rel_path: &str) -> Result<PathBuf, AppError> {
    let relative = sanitize_rel_path(rel_path)?;
    let base = dir
        .canonicalize()
        .map_err(|e| AppError::not_found(&format!("技能目录不可读：{e}")))?;
    let target = base
        .join(relative)
        .canonicalize()
        .map_err(|e| AppError::not_found(&format!("文件不存在或不可读：{e}")))?;

    if !target.starts_with(&base) {
        return Err(AppError::validation_error("文件不在技能目录内"));
    }
    Ok(target)
}

/// Walk a skill directory into a flat, sorted file listing.
///
/// Breadth is bounded by [`MAX_SKILL_FILES`] and [`MAX_SKILL_FILE_DEPTH`];
/// hidden entries, [`SKIPPED_DIRS`] and symlinks are skipped (a symlinked
/// directory could otherwise walk the whole disk, or loop). Unreadable entries
/// are dropped rather than propagated: a listing that shows most of a skill
/// beats an error that shows none of it.
///
/// `SKILL.md` sorts first because it is what the reader came for; the rest is
/// lexicographic, which keeps a directory's files adjacent.
fn collect_skill_files(dir: &Path) -> Vec<SkillFile> {
    let mut files: Vec<SkillFile> = Vec::new();
    let mut queue: Vec<(PathBuf, String, usize)> = vec![(dir.to_path_buf(), String::new(), 0)];

    while let Some((current, prefix, depth)) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            if files.len() >= MAX_SKILL_FILES {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || SKIPPED_DIRS.contains(&name.as_str()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }

            let rel_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };

            if meta.is_dir() {
                if depth + 1 < MAX_SKILL_FILE_DEPTH {
                    queue.push((entry.path(), rel_path, depth + 1));
                }
            } else if meta.is_file() {
                let size = meta.len();
                files.push(SkillFile {
                    readable: size <= MAX_SKILL_FILE_BYTES && !is_binary_path(&rel_path),
                    rel_path,
                    size,
                });
            }
        }
    }

    files.sort_by(|a, b| {
        let rank = |f: &SkillFile| u8::from(f.rel_path != "SKILL.md");
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.rel_path.cmp(&b.rel_path))
    });
    files
}

/// Locate a skill's directory by name, including skills that failed validation.
///
/// Resolution goes through the same discovery the list view uses, so the name
/// the frontend holds is the only thing it has to pass back — no directory
/// path crosses the IPC boundary in the caller's direction, which is what
/// keeps these two commands from being a general-purpose file reader.
fn resolve_skill_dir(
    skill_service: &SkillService,
    name: &str,
    working_dir: Option<&str>,
) -> Result<PathBuf, AppError> {
    let (skills, errors) = skill_service.discover(working_dir.map(Path::new));
    to_skill_infos(skills, errors, &[])
        .into_iter()
        .find(|info| info.name == name)
        .map(|info| info.path)
        .ok_or_else(|| AppError::not_found(&format!("技能不存在：{name}")))
}

/// List the files that make up a skill, for the detail view's file switcher.
///
/// Named rather than pathed (see [`resolve_skill_dir`]); the listing is capped
/// and never fails on individual unreadable entries. Fails only when the name
/// resolves to no discovered skill.
#[tauri::command]
pub async fn skill_files(
    name: String,
    working_dir: Option<String>,
    skill_service: State<'_, Arc<SkillService>>,
) -> Result<Vec<SkillFile>, AppError> {
    let dir = resolve_skill_dir(&skill_service, &name, working_dir.as_deref())?;
    Ok(collect_skill_files(&dir))
}

/// Read one file from a skill's directory as UTF-8 text.
///
/// `rel_path` must be a plain relative path that resolves inside the skill
/// directory ([`resolve_skill_file`]). Oversized files and non-UTF-8 bytes are
/// refused with a structured error rather than truncated or lossily decoded —
/// a half-file shown as if whole is worse than a message saying why not.
#[tauri::command]
pub async fn skill_file_read(
    name: String,
    rel_path: String,
    working_dir: Option<String>,
    skill_service: State<'_, Arc<SkillService>>,
) -> Result<String, AppError> {
    let dir = resolve_skill_dir(&skill_service, &name, working_dir.as_deref())?;
    let target = resolve_skill_file(&dir, &rel_path)?;

    let meta =
        std::fs::metadata(&target).map_err(|e| AppError::not_found(&format!("文件不可读：{e}")))?;
    if !meta.is_file() {
        return Err(AppError::validation_error("目标不是文件"));
    }
    if meta.len() > MAX_SKILL_FILE_BYTES {
        return Err(AppError::validation_error(&format!(
            "文件过大（上限 {} KB）",
            MAX_SKILL_FILE_BYTES / 1024
        )));
    }

    std::fs::read_to_string(&target)
        .map_err(|e| AppError::validation_error(&format!("文件不是 UTF-8 文本：{e}")))
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
    /// everything `skill_list` does apart from the Tauri `State` unwraps and
    /// the settings read (the `disabled` list is injected directly here).
    fn run_with_disabled(
        app: &Path,
        user: &Path,
        working_dir: Option<&Path>,
        disabled: &[String],
    ) -> Vec<SkillInfo> {
        let svc = SkillService::for_test(app.to_path_buf(), user.to_path_buf());
        let (skills, errors) = svc.discover(working_dir);
        to_skill_infos(skills, errors, disabled)
    }

    /// [`run_with_disabled`] with an empty disabled list.
    fn run(app: &Path, user: &Path, working_dir: Option<&Path>) -> Vec<SkillInfo> {
        run_with_disabled(app, user, working_dir, &[])
    }

    /// Owned `Vec<String>` from string literals, for disabled-list fixtures.
    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // No workingDir → user + app-data skills, each with
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

    // A working dir brings the project scope into the result.
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

    // All three scopes empty → an empty list (success).
    #[test]
    fn val_ipc_003_all_scopes_empty_returns_empty() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let infos = run(app.path(), user.path(), Some(proj.path()));
        assert!(infos.is_empty(), "expected empty result: {infos:?}");
    }

    // A validation-failed skill appears with diagnostics, and the
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

    // A same-named skill across scopes collapses to the shadow
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

    // SkillInfo.path is the skill DIRECTORY, not the SKILL.md file.
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

    // Scope serializes to the camelCase literals the wire expects.
    #[test]
    fn val_ipc_007_scope_serializes_to_literals() {
        let info = SkillInfo {
            name: "x".to_string(),
            description: Some("d".to_string()),
            scope: SourceScope::AppData,
            path: PathBuf::from("/skills/x"),
            body: Some("b".to_string()),
            diagnostics: Vec::new(),
            disabled: false,
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

    // A non-existent / relative project working dir is non-fatal —
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

    // Command-level fault → structured AppError. Discovery is
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

        let infos = to_skill_infos(Vec::new(), errors, &[]);
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

    /// Build a [`SettingsService`] over a temp data dir, mirroring how
    /// `skill_set_disabled` / `skill_list` receive it as Tauri state.
    fn settings_service(dir: &TempDir) -> SettingsService {
        let storage = crate::services::StorageService::new(dir.path().to_path_buf()).unwrap();
        SettingsService::new(Arc::new(storage))
    }

    /// Run `skill_list`'s inner logic against the *persisted* settings —
    /// everything the command does apart from the Tauri `State` unwraps.
    fn list_with_settings(app: &Path, user: &Path, settings: &SettingsService) -> Vec<SkillInfo> {
        let svc = SkillService::for_test(app.to_path_buf(), user.to_path_buf());
        let (skills, errors) = svc.discover(None);
        to_skill_infos(
            skills,
            errors,
            &settings.get_settings().unwrap().skills.disabled,
        )
    }

    // `skill_set_disabled`'s pipeline —
    // the service write followed by a `skill_list` read — flips exactly the
    // targeted skill's `disabled` flag, and flips it back on re-enable.
    #[test]
    fn val_config_003_004_set_disabled_round_trips_through_skill_list() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));
        write_skill_raw(user.path(), "beta", &skill_md("b", "b"));
        let settings = settings_service(&data);

        // Disable: alpha flips to true, beta stays false.
        settings.set_skill_disabled("alpha", true).unwrap();
        let idx_owned = list_with_settings(app.path(), user.path(), &settings);
        let idx = by_name(&idx_owned);
        assert!(idx["alpha"].disabled, "alpha must be disabled: {idx:?}");
        assert!(!idx["beta"].disabled, "beta must stay enabled: {idx:?}");

        // Re-enable: alpha flips back to false.
        settings.set_skill_disabled("alpha", false).unwrap();
        let idx_owned = list_with_settings(app.path(), user.path(), &settings);
        let idx = by_name(&idx_owned);
        assert!(!idx["alpha"].disabled, "alpha must be re-enabled: {idx:?}");
        assert!(!idx["beta"].disabled);
    }

    // Disabling two distinct skills shows both as disabled in
    // the skill_list view (read-modify-write keeps the earlier entry).
    #[test]
    fn val_config_006_disabling_two_skills_shows_both_disabled() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));
        write_skill_raw(user.path(), "beta", &skill_md("b", "b"));
        let settings = settings_service(&data);

        settings.set_skill_disabled("alpha", true).unwrap();
        settings.set_skill_disabled("beta", true).unwrap();

        let infos = list_with_settings(app.path(), user.path(), &settings);
        let idx = by_name(&infos);
        assert!(idx["alpha"].disabled, "alpha kept after second disable");
        assert!(idx["beta"].disabled);
    }

    // Empty disabled list → every skill reports disabled=false.
    #[test]
    fn val_config_001_default_all_enabled() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));
        write_skill_raw(user.path(), "beta", &skill_md("b", "b"));

        let infos = run(app.path(), user.path(), None);
        assert_eq!(infos.len(), 2);
        for info in &infos {
            assert!(!info.disabled, "default must be enabled: {info:?}");
        }
    }

    // `disabled` is a definite camelCase boolean wire key.
    #[test]
    fn val_config_002_disabled_is_camel_case_boolean_wire_key() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));

        for disabled in [Vec::new(), names(&["alpha"])] {
            let infos = run_with_disabled(app.path(), user.path(), None, &disabled);
            let json = serde_json::to_value(&infos[0]).unwrap();
            assert!(
                json.get("disabled").is_some_and(|v| v.is_boolean()),
                "wire key `disabled` must be a boolean: {json}"
            );
            assert_eq!(json["disabled"], !disabled.is_empty());
        }
    }

    // An orphan name in the list (skill not discoverable)
    // produces no phantom row and no error.
    #[test]
    fn val_config_008_orphan_name_produces_no_phantom_row() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));

        let infos = run_with_disabled(app.path(), user.path(), None, &names(&["ghost"]));
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 1, "no phantom row for `ghost`: {infos:?}");
        assert!(!idx["alpha"].disabled);
    }

    // A cross-scope shadowed name in the list → exactly one
    // row (the winning scope) with disabled=true, no duplicates.
    #[test]
    fn val_config_009_shadowed_name_single_winner_disabled() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let proj_skills = proj.path().join(".handbox").join("skills");
        write_skill_raw(app.path(), "shared", &skill_md("from app", "a"));
        write_skill_raw(user.path(), "shared", &skill_md("from user", "u"));
        write_skill_raw(&proj_skills, "shared", &skill_md("from proj", "p"));

        let infos = run_with_disabled(
            app.path(),
            user.path(),
            Some(proj.path()),
            &names(&["shared"]),
        );
        assert_eq!(infos.len(), 1, "one winner row only: {infos:?}");
        assert_eq!(infos[0].scope, SourceScope::Project);
        assert!(infos[0].disabled);
    }

    // Duplicate names in the list collapse onto the single
    // skill row, disabled=true, no crash.
    #[test]
    fn val_config_010_duplicate_list_entries_single_row() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));

        let infos = run_with_disabled(app.path(), user.path(), None, &names(&["alpha", "alpha"]));
        assert_eq!(infos.len(), 1, "no duplicate rows: {infos:?}");
        assert!(infos[0].disabled);
    }

    // The global list applies to project-scope (workingDir)
    // skills as well.
    #[test]
    fn val_config_012_disable_applies_to_project_scope() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let proj_skills = proj.path().join(".handbox").join("skills");
        write_skill_raw(&proj_skills, "gamma", &skill_md("from proj", "g"));

        let infos = run_with_disabled(
            app.path(),
            user.path(),
            Some(proj.path()),
            &names(&["gamma"]),
        );
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].scope, SourceScope::Project);
        assert!(infos[0].disabled);
    }

    // Matching is exact-string only — a case-mismatched entry
    // never disables a skill (valid skill names are all-lowercase).
    #[test]
    fn val_config_014_case_mismatched_entry_does_not_disable() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "myskill", &skill_md("m", "m"));

        let infos = run_with_disabled(app.path(), user.path(), None, &names(&["MySkill"]));
        let idx = by_name(&infos);
        assert_eq!(idx.len(), 1);
        assert!(!idx["myskill"].disabled, "exact match only: {infos:?}");
    }

    // The command layer rejects structurally invalid skill names with a
    // structured VALIDATION_ERROR before any settings I/O.
    #[test]
    fn validate_skill_name_rejects_empty_and_whitespace_only() {
        for bad in ["", " ", "  \t ", "\n", "\u{a0}"] {
            let err = validate_skill_name(bad).unwrap_err();
            let json = serde_json::to_value(&err).unwrap();
            assert_eq!(json["code"], "VALIDATION_ERROR", "input {bad:?}");
            assert!(json["message"].is_string());
            assert!(json.get("hint").is_some());
        }
    }

    // Names longer than 256 bytes (measured after trim, on the value
    // that would be stored) are rejected; exactly 256 bytes passes. The
    // multi-byte case checks the limit is bytes, not chars.
    #[test]
    fn validate_skill_name_enforces_256_byte_limit() {
        let max = "a".repeat(256);
        assert_eq!(validate_skill_name(&max).unwrap(), max);

        let over = "a".repeat(257);
        let err = validate_skill_name(&over).unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap()["code"],
            "VALIDATION_ERROR"
        );

        // 86 × 3-byte chars = 258 bytes but only 86 chars.
        let multibyte = "技".repeat(86);
        let err = validate_skill_name(&multibyte).unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap()["code"],
            "VALIDATION_ERROR"
        );

        // Surrounding whitespace is trimmed before measuring.
        let padded = format!("  {}  ", "a".repeat(256));
        assert_eq!(validate_skill_name(&padded).unwrap(), "a".repeat(256));
    }

    // Control characters (C0, DEL, C1) anywhere in the name are rejected.
    #[test]
    fn validate_skill_name_rejects_control_characters() {
        for bad in ["a\nb", "a\tb", "a\u{0}b", "a\u{7f}b", "a\u{9b}b"] {
            let err = validate_skill_name(bad).unwrap_err();
            assert_eq!(
                serde_json::to_value(&err).unwrap()["code"],
                "VALIDATION_ERROR",
                "input {bad:?}"
            );
        }
    }

    // Valid names pass through trimmed, and the trimmed name is what the
    // settings write stores — mirroring `skill_set_disabled`'s body minus the
    // Tauri `State` unwrap. Discovery-produced names have no surrounding
    // whitespace, so exact-match semantics are unaffected.
    #[test]
    fn validate_skill_name_trims_and_round_trips_through_settings() {
        assert_eq!(validate_skill_name("alpha").unwrap(), "alpha");
        assert_eq!(validate_skill_name("  alpha ").unwrap(), "alpha");

        let data = TempDir::new().unwrap();
        let settings = settings_service(&data);
        let name = validate_skill_name("  alpha ").unwrap();
        settings.set_skill_disabled(name, true).unwrap();
        assert_eq!(
            settings.get_settings().unwrap().skills.disabled,
            vec!["alpha"],
            "the stored entry must be the trimmed name"
        );
    }

    // Empty / whitespace-only entries are inert — nothing is
    // falsely disabled and the fold does not panic.
    #[test]
    fn val_config_015_empty_and_whitespace_entries_are_inert() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));
        write_skill_raw(user.path(), "beta", &skill_md("b", "b"));

        let infos = run_with_disabled(app.path(), user.path(), None, &names(&["", "  ", "\t"]));
        assert_eq!(infos.len(), 2);
        for info in &infos {
            assert!(!info.disabled, "whitespace entries must be inert: {info:?}");
        }
    }

    // --- skill file listing / reading -----------------------------------

    // Extension classification is what decides whether the detail view offers a
    // file at all, and it has to hold for uppercase extensions and for the
    // extensionless files skills routinely ship.
    #[test]
    fn is_binary_path_classifies_by_extension_case_insensitively() {
        assert!(is_binary_path("assets/logo.png"));
        assert!(is_binary_path("assets/LOGO.PNG"));
        assert!(!is_binary_path("SKILL.md"));
        assert!(!is_binary_path("scripts/run.sh"));
        assert!(!is_binary_path("Makefile"));
        assert!(!is_binary_path("references/notes.txt"));
    }

    // Every shape that could reach outside the skill directory is refused
    // before any filesystem call.
    #[test]
    fn sanitize_rel_path_rejects_escapes_and_accepts_plain_relatives() {
        assert!(sanitize_rel_path("SKILL.md").is_ok());
        assert!(sanitize_rel_path("references/deep/notes.md").is_ok());

        for bad in [
            "",
            "..",
            "../secrets",
            "references/../../secrets",
            "/etc/passwd",
            "./SKILL.md",
            "refs//notes.md",
            "refs\\notes.md",
            "notes\u{0}.md",
        ] {
            assert!(sanitize_rel_path(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    // A symlink inside the skill directory pointing outside it passes
    // `sanitize_rel_path` (it is a plain name) and must still be refused by the
    // canonicalized containment check.
    #[cfg(unix)]
    #[test]
    fn resolve_skill_file_refuses_a_symlink_escaping_the_skill_dir() {
        let root = TempDir::new().unwrap();
        let dir = root.path().join("alpha");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "body").unwrap();

        let outside = root.path().join("secret.txt");
        fs::write(&outside, "secret").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link.txt")).unwrap();

        assert!(resolve_skill_file(&dir, "SKILL.md").is_ok());
        assert!(
            resolve_skill_file(&dir, "link.txt").is_err(),
            "a symlink resolving outside the skill directory must be refused"
        );
    }

    // The listing shows the whole skill, SKILL.md first, and marks binary
    // assets unreadable while still listing them. Hidden entries and skipped
    // build directories never appear.
    #[test]
    fn collect_skill_files_orders_skill_md_first_and_flags_binaries() {
        let root = TempDir::new().unwrap();
        let dir = root.path().join("alpha");
        fs::create_dir_all(dir.join("references")).unwrap();
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("SKILL.md"), "body").unwrap();
        fs::write(dir.join("logo.png"), [0u8, 1, 2]).unwrap();
        fs::write(dir.join("references/notes.md"), "notes").unwrap();
        fs::write(dir.join("node_modules/ignored.js"), "x").unwrap();
        fs::write(dir.join(".git/config"), "x").unwrap();

        let files = collect_skill_files(&dir);
        let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(paths, vec!["SKILL.md", "logo.png", "references/notes.md"]);

        let logo = files.iter().find(|f| f.rel_path == "logo.png").unwrap();
        assert!(!logo.readable, "a binary asset is listed but not offered");
        assert_eq!(logo.size, 3);
        assert!(files[0].readable);
    }

    // Depth is bounded so a deep tree cannot turn one listing into a full-disk
    // walk; entries past the limit are simply absent.
    #[test]
    fn collect_skill_files_stops_at_the_depth_limit() {
        let root = TempDir::new().unwrap();
        let dir = root.path().join("alpha");
        let mut deep = dir.clone();
        for level in 0..(MAX_SKILL_FILE_DEPTH + 2) {
            deep = deep.join(format!("d{level}"));
        }
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("buried.md"), "x").unwrap();
        fs::write(dir.join("SKILL.md"), "body").unwrap();

        let files = collect_skill_files(&dir);
        assert_eq!(
            files
                .iter()
                .map(|f| f.rel_path.as_str())
                .collect::<Vec<_>>(),
            vec!["SKILL.md"],
            "files below the depth limit must not be listed"
        );
    }

    // A skill that failed validation still resolves: its files are exactly what
    // the reader needs to see in order to fix it.
    #[test]
    fn resolve_skill_dir_finds_valid_and_broken_skills_and_rejects_unknown_names() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        write_skill_raw(app.path(), "alpha", &skill_md("a", "a"));
        write_skill_raw(user.path(), "broken", "no frontmatter at all");
        let service = SkillService::for_test(app.path().to_path_buf(), user.path().to_path_buf());

        assert_eq!(
            resolve_skill_dir(&service, "alpha", None).unwrap(),
            app.path().join("alpha")
        );
        assert_eq!(
            resolve_skill_dir(&service, "broken", None).unwrap(),
            user.path().join("broken")
        );
        assert!(resolve_skill_dir(&service, "missing", None).is_err());
    }
}
