//! Runtime-facing facade over the skill library ([`crate::services::skills`]).
//!
//! Owns the *policy* of which roots exist and their priority, **lowest →
//! highest**: `[(appdata, AppData), (user, User), (project, Project)]`, matching
//! [`discover_skills`]'s contract so a project-scoped skill shadows a same-named
//! user/app-data one.

use std::path::{Path, PathBuf};

use crate::services::skills::{discover_skills, Skill, SkillError, SourceScope};

/// Resolves the skill scope roots and runs discovery over them. The two fixed
/// roots are injected at construction; the project root is derived per call from
/// the run's `working_dir`, so the highest-priority scope tracks whichever
/// directory the agent session is operating in.
pub struct SkillService {
    /// `<app_data_dir>/skills` — lowest priority scope.
    appdata_root: PathBuf,
    /// `~/.agents/skills` — middle priority scope.
    user_root: PathBuf,
}

impl SkillService {
    /// Both roots must already be resolved to absolute paths (production
    /// resolves them from Tauri's `PathResolver`; see `initialize_services`).
    pub fn new(appdata_root: PathBuf, user_root: PathBuf) -> Self {
        Self {
            appdata_root,
            user_root,
        }
    }

    fn project_root(working_dir: Option<&Path>) -> Option<PathBuf> {
        working_dir
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join(".handbox").join("skills"))
    }

    /// Roots in exactly the order [`discover_skills`] expects (earlier = lower
    /// priority). The project entry is omitted when `working_dir` is
    /// `None`/empty.
    pub fn resolve_roots(&self, working_dir: Option<&Path>) -> Vec<(PathBuf, SourceScope)> {
        let mut roots = Vec::with_capacity(3);
        roots.push((self.appdata_root.clone(), SourceScope::AppData));
        roots.push((self.user_root.clone(), SourceScope::User));
        if let Some(project) = Self::project_root(working_dir) {
            roots.push((project, SourceScope::Project));
        }
        roots
    }

    /// `skills` is deduped by name (highest scope wins) and sorted; `errors` are
    /// non-fatal diagnostics the caller may log without aborting the run.
    pub fn discover(&self, working_dir: Option<&Path>) -> (Vec<Skill>, Vec<SkillError>) {
        discover_skills(&self.resolve_roots(working_dir))
    }

    #[cfg(test)]
    pub fn for_test(appdata_root: PathBuf, user_root: PathBuf) -> Self {
        Self::new(appdata_root, user_root)
    }

    /// Inert service for tests that do not exercise skills: the roots do not
    /// exist, and missing roots are silently skipped by discovery.
    #[cfg(test)]
    pub fn empty() -> Self {
        Self::new(
            PathBuf::from("/nonexistent/handbox-skills/appdata"),
            PathBuf::from("/nonexistent/handbox-skills/user"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_skill(root: &Path, dir: &str, description: &str, body: &str) {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\ndescription: {description}\n---\n{body}"),
        )
        .unwrap();
    }

    #[test]
    fn resolve_roots_orders_lowest_to_highest_with_project() {
        let svc = SkillService::new(
            PathBuf::from("/app/skills"),
            PathBuf::from("/home/.agents/skills"),
        );
        let wd = PathBuf::from("/work/proj");
        let roots = svc.resolve_roots(Some(&wd));

        assert_eq!(roots.len(), 3);
        assert_eq!(
            roots[0],
            (PathBuf::from("/app/skills"), SourceScope::AppData)
        );
        assert_eq!(
            roots[1],
            (PathBuf::from("/home/.agents/skills"), SourceScope::User)
        );
        assert_eq!(
            roots[2],
            (
                PathBuf::from("/work/proj/.handbox/skills"),
                SourceScope::Project
            )
        );
    }

    #[test]
    fn resolve_roots_omits_project_when_working_dir_absent_or_empty() {
        let svc = SkillService::new(
            PathBuf::from("/app/skills"),
            PathBuf::from("/home/.agents/skills"),
        );

        let none = svc.resolve_roots(None);
        assert_eq!(none.len(), 2, "no working_dir → no project root");
        assert_eq!(none[0].1, SourceScope::AppData);
        assert_eq!(none[1].1, SourceScope::User);

        let empty = svc.resolve_roots(Some(Path::new("")));
        assert_eq!(empty.len(), 2, "empty working_dir → no project root");
    }

    #[test]
    fn discover_finds_skills_across_scopes_and_dedups() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let proj_skills = proj.path().join(".handbox").join("skills");

        write_skill(app.path(), "alpha", "from app", "app body");
        write_skill(user.path(), "beta", "from user", "user body");
        write_skill(&proj_skills, "alpha", "from proj", "proj body");

        let svc = SkillService::for_test(app.path().to_path_buf(), user.path().to_path_buf());
        let (skills, errors) = svc.discover(Some(proj.path()));
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        let by: std::collections::HashMap<_, _> =
            skills.iter().map(|s| (s.name.as_str(), s)).collect();
        assert_eq!(by.len(), 2);
        assert_eq!(by["alpha"].source.scope, SourceScope::Project);
        assert_eq!(by["alpha"].body, "proj body");
        assert_eq!(by["beta"].source.scope, SourceScope::User);
    }

    #[test]
    fn empty_service_discovers_nothing() {
        let svc = SkillService::empty();
        let (skills, errors) = svc.discover(None);
        assert!(
            skills.is_empty(),
            "empty service finds no skills: {skills:?}"
        );
        assert!(
            errors.is_empty(),
            "missing roots are skipped, not errors: {errors:?}"
        );
    }
}
