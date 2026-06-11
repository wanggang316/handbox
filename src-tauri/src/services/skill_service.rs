//! Runtime-facing facade over the skill library ([`crate::services::skills`]).
//!
//! The discovery module ([`discover_skills`]) is pure over a list of
//! `(root, scope)` pairs; this service owns the *policy* of which three roots
//! exist and in what priority order, and resolves the project root per run from
//! the session's `working_dir`. It is constructed once at startup with the two
//! fixed (app-data + user) roots already resolved to absolute [`PathBuf`]s and
//! injected into [`AgentRuntime`](crate::services::AgentRuntime).
//!
//! Priority is **lowest → highest**: `[(appdata, AppData), (user, User),
//! (project, Project)]`, matching [`discover_skills`]'s documented contract so a
//! project-scoped skill shadows a same-named user/app-data one.

use std::path::{Path, PathBuf};

use crate::services::skills::{discover_skills, Skill, SkillError, SourceScope};

/// Resolves the three skill scope roots and runs discovery over them.
///
/// The app-data root (`<app_data_dir>/skills`) and the user root
/// (`~/.agents/skills`) are fixed for the process lifetime and injected at
/// construction. The project root is derived per call from the run's
/// `working_dir` (`<working_dir>/.handbox/skills`), so the highest-priority
/// scope tracks whichever directory the agent session is operating in.
pub struct SkillService {
    /// `<app_data_dir>/skills` — lowest priority scope.
    appdata_root: PathBuf,
    /// `~/.agents/skills` — middle priority scope.
    user_root: PathBuf,
}

impl SkillService {
    /// Construct with the two fixed roots already resolved to absolute paths.
    /// The production caller resolves these from Tauri's `PathResolver`
    /// (`app_data_dir()` + `home_dir()`); see `initialize_services`.
    pub fn new(appdata_root: PathBuf, user_root: PathBuf) -> Self {
        Self {
            appdata_root,
            user_root,
        }
    }

    /// The project skill root for a given working directory, or `None` when the
    /// working directory is absent or empty. `<working_dir>/.handbox/skills`.
    fn project_root(working_dir: Option<&Path>) -> Option<PathBuf> {
        working_dir
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join(".handbox").join("skills"))
    }

    /// Resolve the scope roots for a run in **lowest → highest** priority order:
    /// `[(appdata, AppData), (user, User), (project, Project)]`. The project
    /// entry is omitted when `working_dir` is `None`/empty.
    ///
    /// Returned in exactly the order [`discover_skills`] expects (earlier =
    /// lower priority), so a project skill shadows a same-named user/app-data
    /// one during dedup.
    pub fn resolve_roots(&self, working_dir: Option<&Path>) -> Vec<(PathBuf, SourceScope)> {
        let mut roots = Vec::with_capacity(3);
        roots.push((self.appdata_root.clone(), SourceScope::AppData));
        roots.push((self.user_root.clone(), SourceScope::User));
        if let Some(project) = Self::project_root(working_dir) {
            roots.push((project, SourceScope::Project));
        }
        roots
    }

    /// Discover all skills across the resolved scope roots for this run.
    ///
    /// Thin pass-through to [`discover_skills`]: returns `(skills, errors)`
    /// where `skills` is deduped by name (highest scope wins) and sorted
    /// alphabetically, and `errors` are non-fatal diagnostics the caller may
    /// log without aborting the run.
    pub fn discover(&self, working_dir: Option<&Path>) -> (Vec<Skill>, Vec<SkillError>) {
        discover_skills(&self.resolve_roots(working_dir))
    }

    /// Test-only constructor injecting an explicit root list. Lets unit tests
    /// point the two fixed roots at tempdirs (or use [`SkillService::empty`])
    /// without resolving real OS paths.
    #[cfg(test)]
    pub fn for_test(appdata_root: PathBuf, user_root: PathBuf) -> Self {
        Self::new(appdata_root, user_root)
    }

    /// Test-only constructor whose fixed roots point at non-existent paths, so
    /// discovery finds nothing from app-data/user (missing roots are silently
    /// skipped). Used to inject an inert skill service into the agent-runtime
    /// tests that do not exercise skills.
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

    /// Write `<root>/<dir>/SKILL.md` with frontmatter description + body.
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

        // alpha is shadowed by the project scope (highest priority); beta is user.
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
