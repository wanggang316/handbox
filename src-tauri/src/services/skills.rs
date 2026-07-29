//! Skills are markdown files describing optional capabilities the model can use.
//!
//! A skill is a directory containing a `SKILL.md` file. The file's YAML
//! frontmatter declares metadata (`name`, `description`,
//! `disable-model-invocation`); the body is the prose injected into the system
//! prompt's "Skills" section when the skill is enabled.
//!
//! [`validate`] is filesystem-independent (it takes already-parsed inputs) so
//! it can be unit-tested in isolation; [`discover_skills`] walks the scope
//! roots, parses each `SKILL.md`, deduplicates by name, and feeds the
//! survivors through it.

use crate::utils::frontmatter::{parse_frontmatter, FrontmatterError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Skill name length cap (bytes) per the Agent Skills spec.
const MAX_NAME_LENGTH: usize = 64;
/// Description length cap (bytes) per the Agent Skills spec.
const MAX_DESCRIPTION_LENGTH: usize = 1024;

/// The scope a skill was discovered in. Higher-priority scopes shadow
/// lower-priority scopes by canonical name during dedup.
///
/// `Ord` makes `Project` the greatest variant so `max`/sort picks the
/// highest-priority source. The serde form (`"project"` / `"user"` /
/// `"appData"`) is a contract with downstream IPC/UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceScope {
    /// Application-bundled defaults — lowest priority.
    AppData,
    /// User-level skills, typically `~/.agents/skills/`.
    User,
    /// Project-level skills, typically `<wd>/.handbox/skills/` — highest priority.
    Project,
}

/// Where a skill was loaded from; `path` points at the `SKILL.md` file or its
/// directory and is carried only for error location.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceInfo {
    pub scope: SourceScope,
    pub path: PathBuf,
}

/// Parsed YAML frontmatter on a `SKILL.md`.
///
/// Unknown fields are tolerated so skills can carry forward-compatible
/// metadata. All fields default, so an empty frontmatter block (`Value::Null`)
/// deserializes into the default.
#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct SkillMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "disable-model-invocation")]
    pub disable_model_invocation: bool,
}

/// A discovered, validated skill.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    /// Canonical name (lowercase ASCII alphanumeric + dashes). Derived from
    /// the parent directory; the frontmatter `name` field, when present, must
    /// equal it.
    pub name: String,
    /// Description shown to the model when listing available skills. Preserved
    /// verbatim from the frontmatter (not trimmed).
    pub description: String,
    /// Body of `SKILL.md` — everything after the frontmatter close.
    pub body: String,
    /// True when the model should NOT auto-invoke this skill (it can only be
    /// invoked explicitly, e.g., via `/skill:<name>`).
    pub disable_model_invocation: bool,
    pub source: SourceInfo,
}

/// Errors raised while loading and validating a `SKILL.md`. Every variant
/// carries the offending `path` for diagnostics.
#[derive(Debug, Error)]
pub enum SkillError {
    /// IO error (e.g. a scope root that is a regular file, or a `SKILL.md` that
    /// cannot be read). Collected so one unreadable entry does not abort
    /// discovery. Separate from [`SkillError::Loader`] because
    /// [`FrontmatterError`] has no IO representation and `std::io::Error` is
    /// not `Clone`.
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Frontmatter parse error from the underlying loader (malformed YAML, an
    /// unterminated block, or a field whose YAML type doesn't match the
    /// schema — e.g. a non-boolean `disable-model-invocation`).
    #[error("loader error in {path}: {source}")]
    Loader {
        path: PathBuf,
        #[source]
        source: FrontmatterError,
    },
    #[error("missing required field `description` in {path}")]
    MissingDescription { path: PathBuf },
    #[error("description exceeds {max} bytes ({actual}) in {path}")]
    DescriptionTooLong {
        path: PathBuf,
        actual: usize,
        max: usize,
    },
    #[error(
        "frontmatter `name` ({frontmatter_name:?}) doesn't match directory name ({dir_name:?}) at {path}"
    )]
    NameMismatch {
        path: PathBuf,
        frontmatter_name: String,
        dir_name: String,
    },
    /// Skill name fails validation (must be lowercase ASCII alphanumeric +
    /// dashes, no leading/trailing dash, no consecutive dashes, max 64 bytes).
    #[error("invalid skill name {name:?} at {path}: {reason}")]
    InvalidName {
        path: PathBuf,
        name: String,
        reason: String,
    },
}

/// Validate parsed skill inputs and turn them into a [`Skill`].
///
/// `metadata` is `None` when there was no frontmatter block at all; an empty
/// block deserializes to `Some(SkillMetadata::default())`.
///
/// Validation order is fixed: description-required → description-length →
/// name-mismatch → name-valid. Only the first violation is reported.
pub fn validate(
    dir_name: String,
    metadata: Option<SkillMetadata>,
    body: String,
    source: SourceInfo,
) -> Result<Skill, SkillError> {
    let path = source.path.clone();
    let metadata = metadata.unwrap_or_default();

    // Description is required. Emptiness is judged on the trimmed value;
    // length, deliberately, on the untrimmed original (parity with upstream).
    let description = match metadata.description {
        Some(desc) if !desc.trim().is_empty() => desc,
        _ => return Err(SkillError::MissingDescription { path }),
    };

    if description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(SkillError::DescriptionTooLong {
            path,
            actual: description.len(),
            max: MAX_DESCRIPTION_LENGTH,
        });
    }

    let name = match metadata.name {
        Some(frontmatter_name) => {
            if frontmatter_name != dir_name {
                return Err(SkillError::NameMismatch {
                    path,
                    frontmatter_name,
                    dir_name,
                });
            }
            frontmatter_name
        }
        None => dir_name,
    };

    if let Err(reason) = validate_name(&name) {
        return Err(SkillError::InvalidName {
            path,
            name,
            reason: reason.to_string(),
        });
    }

    Ok(Skill {
        name,
        description,
        body,
        disable_model_invocation: metadata.disable_model_invocation,
        source,
    })
}

/// Validate a skill name per the Agent Skills spec. Length is measured in
/// bytes (`str::len`), not chars.
fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("name must not be empty");
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err("name exceeds 64 bytes");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)");
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("name must not start or end with a hyphen");
    }
    if name.contains("--") {
        return Err("name must not contain consecutive hyphens");
    }
    Ok(())
}

/// A skill that loaded cleanly (file read + frontmatter parsed) but is not yet
/// validated. This is the unit deduplicated by name.
///
/// Load and validation are two passes so a name slot is claimed only by an
/// entry that LOADED: a higher-priority entry with broken frontmatter leaves a
/// lower-priority same-named good skill visible, while one that loads but then
/// fails validation does claim the slot and shadows it, reporting its
/// diagnostic in its place.
struct LoadedSkill {
    /// Canonical name, derived from the parent directory basename.
    name: String,
    metadata: Option<SkillMetadata>,
    body: String,
    source: SourceInfo,
}

/// Discover `SKILL.md` files across the given scope roots and validate them.
///
/// `roots` is ordered from **lowest to highest** priority — callers pass
/// `[(appdata, AppData), (user, User), (project, Project)]`. Only immediate
/// subdirectories containing a `SKILL.md` are considered (non-recursive); a
/// missing root is silently skipped, while any other IO error and every
/// frontmatter parse error is collected rather than fatal.
///
/// Deduplication happens **after a successful load but before validation**,
/// via a `BTreeMap` keyed by name: a later (higher-priority) root overwrites an
/// earlier same-named entry, and the map also yields the alphabetical order.
///
/// Returns `(skills, errors)`. Discovery is lenient: a single bad skill yields
/// a diagnostic but never aborts the walk, and the function never panics.
pub fn discover_skills(roots: &[(PathBuf, SourceScope)]) -> (Vec<Skill>, Vec<SkillError>) {
    let mut by_name: BTreeMap<String, LoadedSkill> = BTreeMap::new();
    let mut errors: Vec<SkillError> = Vec::new();

    for (root, scope) in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            // A scope whose directory doesn't exist is simply not configured.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            // Any other IO error on the root (e.g. it's a regular file, or
            // permission denied) is a diagnostic but not fatal.
            Err(err) => {
                errors.push(SkillError::Io {
                    path: root.clone(),
                    source: err,
                });
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    errors.push(SkillError::Io {
                        path: root.clone(),
                        source: err,
                    });
                    continue;
                }
            };

            let dir_path = entry.path();

            // Non-recursive: only immediate subdirectories are skill candidates.
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => {}
                Ok(_) => continue,
                Err(err) => {
                    errors.push(SkillError::Io {
                        path: dir_path,
                        source: err,
                    });
                    continue;
                }
            }

            let candidate = dir_path.join("SKILL.md");
            if !candidate.is_file() {
                continue;
            }

            match load_skill(&candidate, *scope) {
                Ok(loaded) => {
                    // Insert after a SUCCESSFUL load, before validation: a
                    // higher-priority root overwrites the same-named slot.
                    by_name.insert(loaded.name.clone(), loaded);
                }
                Err(err) => errors.push(err),
            }
        }
    }

    let mut skills: Vec<Skill> = Vec::with_capacity(by_name.len());
    for loaded in by_name.into_values() {
        match validate(loaded.name, loaded.metadata, loaded.body, loaded.source) {
            Ok(skill) => skills.push(skill),
            Err(err) => errors.push(err),
        }
    }

    (skills, errors)
}

/// Read and parse a single `SKILL.md`, deriving its canonical name from the
/// parent directory.
fn load_skill(path: &Path, scope: SourceScope) -> Result<LoadedSkill, SkillError> {
    let content = std::fs::read_to_string(path).map_err(|err| SkillError::Io {
        path: path.to_path_buf(),
        source: err,
    })?;

    let parsed =
        parse_frontmatter::<SkillMetadata>(&content).map_err(|err| SkillError::Loader {
            path: path.to_path_buf(),
            source: err,
        })?;

    let name = parent_dir_name(path).ok_or_else(|| SkillError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "SKILL.md has no usable parent directory name",
        ),
    })?;

    Ok(LoadedSkill {
        name,
        metadata: parsed.metadata,
        body: parsed.body,
        source: SourceInfo {
            scope,
            path: path.to_path_buf(),
        },
    })
}

/// The lossy UTF-8 basename of `path`'s parent directory, or `None` when there
/// is no parent (which cannot happen for a `<root>/<dir>/SKILL.md` candidate).
fn parent_dir_name(path: &Path) -> Option<String> {
    path.parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
}

/// Format the available-skills index for the system prompt as an
/// `<available_skills>` XML block. Each skill becomes a `<skill>` element with
/// `<name>` and `<description>` children only.
///
/// No `<location>` line and no filesystem path is ever emitted: the model
/// invokes a skill by name through the skill tool, and the guidance prose says
/// exactly that — it must not contain a `read tool to load` instruction.
///
/// `disable_model_invocation` skills are still listed but tagged
/// `<skill opt-in="true">` so the model knows not to auto-invoke them.
///
/// Returns `None` for an empty list (no section emitted at all). Skills are
/// sorted alphabetically by name for byte-deterministic output.
pub fn format_skills_section(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut sorted: Vec<&Skill> = skills.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out = String::new();
    out.push_str("The following skills provide specialized instructions for specific tasks.\n");
    out.push_str(
        "When a task matches a skill's description, call the skill tool with that skill's name \
         to load its full instructions.\n\n",
    );
    out.push_str("<available_skills>\n");
    for skill in sorted {
        if skill.disable_model_invocation {
            out.push_str("  <skill opt-in=\"true\">\n");
        } else {
            out.push_str("  <skill>\n");
        }
        out.push_str(&format!("    <name>{}</name>\n", escape_xml(&skill.name)));
        out.push_str(&format!(
            "    <description>{}</description>\n",
            escape_xml(&skill.description)
        ));
        out.push_str("  </skill>\n");
    }
    out.push_str("</available_skills>");
    Some(out)
}

/// Minimal XML entity escape for the skills section: `&`, `<`, `>`, `"`, `'`.
///
/// Each input `&` is matched as a literal character (the inserted entity is
/// never re-scanned), so already-emitted entities are not double-escaped.
/// Newlines pass through verbatim, keeping multi-line descriptions intact.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> SourceInfo {
        SourceInfo {
            scope: SourceScope::Project,
            path: PathBuf::from("/skills/test/SKILL.md"),
        }
    }

    fn meta(name: Option<&str>, description: Option<&str>) -> SkillMetadata {
        SkillMetadata {
            name: name.map(str::to_string),
            description: description.map(str::to_string),
            disable_model_invocation: false,
        }
    }

    /// Run `validate` with a directory name and frontmatter, default source.
    fn run(dir_name: &str, metadata: Option<SkillMetadata>) -> Result<Skill, SkillError> {
        validate(dir_name.to_string(), metadata, "body".to_string(), source())
    }

    // A legal name passes; with no frontmatter.name the directory name is used.
    #[test]
    fn val_001_valid_name_passes() {
        let skill = run(
            "valid-skill",
            Some(meta(None, Some("A valid description."))),
        )
        .unwrap();
        assert_eq!(skill.name, "valid-skill");
        assert_eq!(skill.description, "A valid description.");
        assert!(!skill.disable_model_invocation);
        assert_eq!(skill.source.scope, SourceScope::Project);
        assert_eq!(skill.body, "body");
    }

    // Assorted legal names all pass through the dir-name fallback.
    #[test]
    fn val_001_valid_name_variants_pass() {
        for name in ["valid-name", "valid", "v123", "a", "a-b-c", "0-9"] {
            let res = run(name, Some(meta(None, Some("ok"))));
            assert!(res.is_ok(), "expected ok for {name:?}, got {res:?}");
            assert_eq!(res.unwrap().name, name);
        }
    }

    // Each name violation, via the dir-fallback path, yields InvalidName with a
    // reason naming its category.
    #[test]
    fn val_002_dir_fallback_invalid_name_categories() {
        let cases: &[(&str, &str)] = &[
            ("", "empty"),
            ("-leading", "hyphen"),
            ("trailing-", "hyphen"),
            ("bad--name", "consecutive"),
            ("Upper", "invalid characters"),
            ("under_score", "invalid characters"),
        ];
        for (dir, reason_fragment) in cases {
            let res = run(dir, Some(meta(None, Some("ok"))));
            match res {
                Err(SkillError::InvalidName { name, reason, .. }) => {
                    assert_eq!(&name, dir, "name field should echo dir for {dir:?}");
                    assert!(
                        reason.contains(reason_fragment),
                        "reason {reason:?} should mention {reason_fragment:?} for {dir:?}",
                    );
                }
                other => panic!("expected InvalidName for {dir:?}, got {other:?}"),
            }
        }
    }

    // An oversized dir name yields InvalidName with the length reason, not the
    // NameMismatch that a >64-byte frontmatter name would take.
    #[test]
    fn val_002_dir_fallback_too_long_is_invalid_name() {
        let long = "a".repeat(MAX_NAME_LENGTH + 1);
        match run(&long, Some(meta(None, Some("ok")))) {
            Err(SkillError::InvalidName { reason, .. }) => {
                assert!(reason.contains("64 bytes"), "unexpected reason: {reason:?}");
            }
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    // A 64-byte name passes, 65 bytes fails — the limit is bytes, not chars.
    #[test]
    fn val_003_name_length_byte_boundary() {
        let ok = "a".repeat(MAX_NAME_LENGTH);
        assert_eq!(ok.len(), 64);
        assert!(run(&ok, Some(meta(None, Some("ok")))).is_ok());

        let too_long = "a".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(too_long.len(), 65);
        assert!(matches!(
            run(&too_long, Some(meta(None, Some("ok")))),
            Err(SkillError::InvalidName { .. })
        ));
    }

    // The name cap is bytes, but the legal charset is ASCII where bytes ≡ chars,
    // so no *legal* name can exceed 64 bytes at ≤ 64 chars: any multi-byte name
    // trips the charset check FIRST. This pins that ordering. frontmatter.name
    // == dir_name keeps us on the name-validation path (avoids NameMismatch).
    #[test]
    fn val_003_multibyte_name_hits_charset_before_length() {
        let name = "é".repeat(32);
        // 64 bytes (length alone would pass) but only 32 chars.
        assert_eq!(name.len(), 64);
        assert_eq!(name.chars().count(), 32);
        assert_eq!(name.len(), MAX_NAME_LENGTH);
        match run(&name, Some(meta(Some(&name), Some("ok")))) {
            Err(SkillError::InvalidName {
                name: got, reason, ..
            }) => {
                assert_eq!(got, name);
                assert!(
                    reason.contains("invalid characters"),
                    "expected charset reason, got {reason:?}",
                );
            }
            other => panic!("expected InvalidName (charset), got {other:?}"),
        }
    }

    #[test]
    fn val_004_missing_description() {
        match run("ok-skill", Some(meta(None, None))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    #[test]
    fn val_004_whitespace_only_description() {
        match run("ok-skill", Some(meta(None, Some("   \n\t  ")))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    #[test]
    fn val_004_yaml_null_description() {
        let m: SkillMetadata = serde_yaml::from_str("name: ok-skill\ndescription: null\n").unwrap();
        assert!(m.description.is_none());
        match run("ok-skill", Some(m)) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // A 1024-byte description passes, 1025 fails. The name must be valid for the
    // length check to be reached at all.
    #[test]
    fn val_005_description_length_byte_boundary() {
        let ok = "x".repeat(MAX_DESCRIPTION_LENGTH);
        assert_eq!(ok.len(), 1024);
        let skill = run("ok-skill", Some(meta(None, Some(&ok)))).unwrap();
        assert_eq!(skill.description.len(), 1024);

        let too_long = "x".repeat(MAX_DESCRIPTION_LENGTH + 1);
        assert_eq!(too_long.len(), 1025);
        match run("ok-skill", Some(meta(None, Some(&too_long)))) {
            Err(SkillError::DescriptionTooLong { actual, max, .. }) => {
                assert_eq!(actual, 1025);
                assert_eq!(max, MAX_DESCRIPTION_LENGTH);
            }
            other => panic!("expected DescriptionTooLong, got {other:?}"),
        }
    }

    // The description cap is BYTES, not chars: 513 two-byte chars is 1026 bytes
    // but only 513 chars, which a char-counting cap would wrongly accept.
    #[test]
    fn val_005_description_length_is_bytes_not_chars() {
        let desc = "é".repeat(513);
        assert_eq!(desc.len(), 1026);
        assert_eq!(desc.chars().count(), 513);
        assert!(desc.chars().count() < MAX_DESCRIPTION_LENGTH);
        match run("ok-skill", Some(meta(None, Some(&desc)))) {
            Err(SkillError::DescriptionTooLong { actual, max, .. }) => {
                assert_eq!(actual, 1026);
                assert_eq!(max, MAX_DESCRIPTION_LENGTH);
            }
            other => panic!("expected DescriptionTooLong, got {other:?}"),
        }
    }

    #[test]
    fn val_006_name_mismatch() {
        match run(
            "name-mismatch",
            Some(meta(Some("different-name"), Some("ok"))),
        ) {
            Err(SkillError::NameMismatch {
                frontmatter_name,
                dir_name,
                ..
            }) => {
                assert_eq!(frontmatter_name, "different-name");
                assert_eq!(dir_name, "name-mismatch");
            }
            other => panic!("expected NameMismatch, got {other:?}"),
        }
    }

    #[test]
    fn val_007_order_missing_description_beats_name_mismatch() {
        match run("dir-name", Some(meta(Some("other-name"), None))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription first, got {other:?}"),
        }
    }

    #[test]
    fn val_007_order_too_long_beats_name_mismatch() {
        let too_long = "x".repeat(MAX_DESCRIPTION_LENGTH + 1);
        match run("dir-name", Some(meta(Some("other-name"), Some(&too_long)))) {
            Err(SkillError::DescriptionTooLong { actual, .. }) => {
                assert_eq!(actual, 1025);
            }
            other => panic!("expected DescriptionTooLong first, got {other:?}"),
        }
    }

    // InvalidName is only reachable when frontmatter.name == dir_name; a
    // differing name that is itself invalid is still reported as NameMismatch.
    #[test]
    fn val_007_mismatch_beats_invalid_when_names_differ() {
        match run("good-dir", Some(meta(Some("Bad_Name"), Some("ok")))) {
            Err(SkillError::NameMismatch { .. }) => {}
            other => panic!("expected NameMismatch, got {other:?}"),
        }
    }

    // Equal-but-invalid names pass the mismatch check, then fail name-valid.
    #[test]
    fn val_007_invalid_name_via_matching_invalid_frontmatter() {
        match run("Bad_Name", Some(meta(Some("Bad_Name"), Some("ok")))) {
            Err(SkillError::InvalidName { name, .. }) => {
                assert_eq!(name, "Bad_Name");
            }
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    #[test]
    fn val_008_no_frontmatter_missing_description() {
        match run("ok-skill", None) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    #[test]
    fn val_008_empty_yaml_block_missing_description() {
        // An empty block round-trips to the default struct (all fields are
        // #[serde(default)]); discovery passes exactly this `Some(default)`.
        let from_null: SkillMetadata = serde_yaml::from_value(serde_yaml::Value::Null).unwrap();
        assert_eq!(from_null, SkillMetadata::default());
        match run("ok-skill", Some(from_null)) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // frontmatter.name == dir_name keeps these on the charset path rather than
    // the NameMismatch one.
    #[test]
    fn val_009_non_ascii_and_punctuation_names_invalid() {
        for bad in ["café", "skill.name", "foo/bar", "naïve", "世界"] {
            match run(bad, Some(meta(Some(bad), Some("ok")))) {
                Err(SkillError::InvalidName { name, reason, .. }) => {
                    assert_eq!(&name, bad);
                    assert!(
                        reason.contains("invalid characters"),
                        "reason {reason:?} for {bad:?}",
                    );
                }
                other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
            }
        }
    }

    // Length uses the UNTRIMMED original: 1024 bytes plus a trailing space is
    // TooLong even though the trimmed value is exactly 1024.
    #[test]
    fn val_010_length_uses_untrimmed_value() {
        let mut desc = "x".repeat(MAX_DESCRIPTION_LENGTH);
        desc.push(' ');
        assert_eq!(desc.len(), 1025);
        assert_eq!(desc.trim().len(), 1024);
        match run("ok-skill", Some(meta(None, Some(&desc)))) {
            Err(SkillError::DescriptionTooLong { actual, .. }) => {
                assert_eq!(actual, 1025);
            }
            other => panic!("expected DescriptionTooLong, got {other:?}"),
        }
    }

    #[test]
    fn val_011_description_preserved_verbatim() {
        let desc = "  surrounded by spaces  \nand a newline  ";
        let skill = run("ok-skill", Some(meta(None, Some(desc)))).unwrap();
        assert_eq!(skill.description, desc);
    }

    // The camelCase literals are a contract with downstream IPC/UI.
    #[test]
    fn source_scope_serializes_to_camel_case() {
        assert_eq!(
            serde_json::to_string(&SourceScope::Project).unwrap(),
            "\"project\""
        );
        assert_eq!(
            serde_json::to_string(&SourceScope::User).unwrap(),
            "\"user\""
        );
        assert_eq!(
            serde_json::to_string(&SourceScope::AppData).unwrap(),
            "\"appData\""
        );
        let p: SourceScope = serde_json::from_str("\"project\"").unwrap();
        assert_eq!(p, SourceScope::Project);
    }

    // Project is the greatest variant so dedup can pick the winner with `max`.
    #[test]
    fn source_scope_priority_order() {
        assert!(SourceScope::AppData < SourceScope::User);
        assert!(SourceScope::User < SourceScope::Project);
        assert_eq!(
            [
                SourceScope::User,
                SourceScope::AppData,
                SourceScope::Project
            ]
            .into_iter()
            .max(),
            Some(SourceScope::Project)
        );
    }

    #[test]
    fn disable_model_invocation_kebab_rename_and_default() {
        let on: SkillMetadata =
            serde_yaml::from_str("description: x\ndisable-model-invocation: true\n").unwrap();
        assert!(on.disable_model_invocation);
        let skill = run("ok-skill", Some(on)).unwrap();
        assert!(skill.disable_model_invocation);

        let off: SkillMetadata = serde_yaml::from_str("description: x\n").unwrap();
        assert!(!off.disable_model_invocation);
    }

    use std::fs;
    use tempfile::TempDir;

    /// Write `<root>/<dir>/SKILL.md` with `content`, creating parents.
    fn write_skill(root: &Path, dir: &str, content: &str) -> PathBuf {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        let file = skill_dir.join("SKILL.md");
        fs::write(&file, content).unwrap();
        file
    }

    /// Minimal valid SKILL.md body with a description and matching/no name.
    fn skill_md(description: &str, body: &str) -> String {
        format!("---\ndescription: {description}\n---\n{body}")
    }

    /// Convenience: the three real scope roots in priority order, lowest first.
    fn roots(appdata: &Path, user: &Path, project: &Path) -> Vec<(PathBuf, SourceScope)> {
        vec![
            (appdata.to_path_buf(), SourceScope::AppData),
            (user.to_path_buf(), SourceScope::User),
            (project.to_path_buf(), SourceScope::Project),
        ]
    }

    // A skill's name comes from its parent directory and its scope from the root
    // it was found under.
    #[test]
    fn val_discovery_001_three_scopes_discovered() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        write_skill(app.path(), "alpha", &skill_md("from app", "a body"));
        write_skill(user.path(), "beta", &skill_md("from user", "b body"));
        write_skill(proj.path(), "gamma", &skill_md("from proj", "g body"));

        let (skills, errors) = discover_skills(&roots(app.path(), user.path(), proj.path()));
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        let by: std::collections::HashMap<_, _> =
            skills.iter().map(|s| (s.name.as_str(), s)).collect();
        assert_eq!(by.len(), 3);
        assert_eq!(by["alpha"].source.scope, SourceScope::AppData);
        assert_eq!(by["beta"].source.scope, SourceScope::User);
        assert_eq!(by["gamma"].source.scope, SourceScope::Project);
        assert_eq!(by["alpha"].description, "from app");
        assert_eq!(by["gamma"].body, "g body");
    }

    // The winner's body, description and scope all come from the highest-priority
    // root that supplied the name.
    #[test]
    fn val_discovery_002_same_name_project_shadows_user_shadows_appdata() {
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        write_skill(app.path(), "shared", &skill_md("from app", "app body"));
        write_skill(user.path(), "shared", &skill_md("from user", "user body"));
        write_skill(proj.path(), "shared", &skill_md("from proj", "proj body"));

        let (skills, errors) = discover_skills(&roots(app.path(), user.path(), proj.path()));
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(skills.len(), 1, "dedup should collapse to one: {skills:?}");
        assert_eq!(skills[0].name, "shared");
        assert_eq!(skills[0].source.scope, SourceScope::Project);
        assert_eq!(skills[0].description, "from proj");
        assert_eq!(skills[0].body, "proj body");

        // And User shadows AppData when Project is absent.
        let app2 = TempDir::new().unwrap();
        let user2 = TempDir::new().unwrap();
        write_skill(app2.path(), "shared", &skill_md("from app", "app body"));
        write_skill(user2.path(), "shared", &skill_md("from user", "user body"));
        let (skills2, errors2) = discover_skills(&[
            (app2.path().to_path_buf(), SourceScope::AppData),
            (user2.path().to_path_buf(), SourceScope::User),
        ]);
        assert!(errors2.is_empty(), "unexpected errors: {errors2:?}");
        assert_eq!(skills2.len(), 1);
        assert_eq!(skills2[0].source.scope, SourceScope::User);
        assert_eq!(skills2[0].description, "from user");
    }

    // A missing scope directory is not an error; results stay sorted by name.
    #[test]
    fn val_discovery_003_missing_scope_silently_skipped() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        // `user` root is never created.
        let user_missing = app.path().join("does-not-exist-user-root");
        write_skill(app.path(), "zeta", &skill_md("z", "z"));
        write_skill(proj.path(), "alpha", &skill_md("a", "a"));

        let (skills, errors) = discover_skills(&[
            (app.path().to_path_buf(), SourceScope::AppData),
            (user_missing, SourceScope::User),
            (proj.path().to_path_buf(), SourceScope::Project),
        ]);
        assert!(errors.is_empty(), "missing dir must not error: {errors:?}");
        let names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    // A SKILL.md one level deeper than the immediate subdirectory is not found.
    #[test]
    fn val_discovery_004_non_recursive() {
        let root = TempDir::new().unwrap();
        // `<root>/outer/inner/SKILL.md` but no `<root>/outer/SKILL.md`.
        let inner = root.path().join("outer").join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("SKILL.md"), skill_md("nested", "n")).unwrap();

        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        assert!(
            skills.is_empty(),
            "nested skill must not be found: {skills:?}"
        );
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn val_discovery_005_stray_file_and_dir_without_skill_md_ignored() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("README.md"), "not a skill").unwrap();
        fs::create_dir_all(root.path().join("empty-dir")).unwrap();
        // The candidate match is case-insensitive on macOS/Windows, so this
        // wrong name must differ by more than case to be portable.
        let wrong = root.path().join("wrong");
        fs::create_dir_all(&wrong).unwrap();
        fs::write(wrong.join("NOTES.md"), skill_md("x", "x")).unwrap();
        // One real skill so we know discovery itself works.
        write_skill(root.path(), "real", &skill_md("real desc", "real body"));

        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "real");
    }

    // Sorting holds regardless of insertion or directory-iteration order.
    #[test]
    fn val_discovery_006_sorted_by_name() {
        let root = TempDir::new().unwrap();
        for n in ["mango", "apple", "zebra", "banana"] {
            write_skill(root.path(), n, &skill_md("d", "b"));
        }
        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["apple", "banana", "mango", "zebra"]);
    }

    // A higher-priority entry that fails to LOAD never claims the name slot, so
    // the lower-priority same-named good skill still surfaces — and the load
    // error is reported alongside it.
    #[test]
    fn val_discovery_007_loader_bad_does_not_shadow_lower_priority() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        write_skill(app.path(), "dup", &skill_md("good from app", "app body"));
        // Higher-priority but with broken frontmatter — the load fails.
        let bad_path = write_skill(proj.path(), "dup", "---\nname: : :\n---\nbody");

        let (skills, errors) = discover_skills(&[
            (app.path().to_path_buf(), SourceScope::AppData),
            (proj.path().to_path_buf(), SourceScope::Project),
        ]);
        assert_eq!(
            skills.len(),
            1,
            "low-priority good skill should remain: {skills:?}"
        );
        assert_eq!(skills[0].name, "dup");
        assert_eq!(skills[0].source.scope, SourceScope::AppData);
        assert_eq!(skills[0].description, "good from app");
        assert_eq!(errors.len(), 1, "expected one loader error: {errors:?}");
        match &errors[0] {
            SkillError::Loader { path, source } => {
                assert_eq!(path, &bad_path);
                assert!(matches!(source, FrontmatterError::InvalidYaml(_)));
            }
            other => panic!("expected Loader error, got {other:?}"),
        }
    }

    // A higher-priority entry that LOADS but fails VALIDATION does occupy the
    // name slot, shadowing the lower-priority good skill; its validation error
    // is reported in that skill's place.
    #[test]
    fn val_discovery_008_validate_bad_shadows_lower_priority() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        write_skill(app.path(), "dup", &skill_md("good from app", "app body"));
        // Higher-priority, loads fine, but its `name` mismatches the directory.
        let bad_path = write_skill(
            proj.path(),
            "dup",
            "---\nname: not-dup\ndescription: d\n---\nbody",
        );

        let (skills, errors) = discover_skills(&[
            (app.path().to_path_buf(), SourceScope::AppData),
            (proj.path().to_path_buf(), SourceScope::Project),
        ]);
        assert!(
            skills.is_empty(),
            "validate-bad winner should shadow the lower skill: {skills:?}"
        );
        assert_eq!(errors.len(), 1, "expected one validation error: {errors:?}");
        match &errors[0] {
            SkillError::NameMismatch {
                path,
                frontmatter_name,
                dir_name,
            } => {
                assert_eq!(path, &bad_path);
                assert_eq!(frontmatter_name, "not-dup");
                assert_eq!(dir_name, "dup");
            }
            other => panic!("expected NameMismatch error, got {other:?}"),
        }
    }

    // A non-boolean `disable-model-invocation` is a Loader error and does not
    // abort discovery of the other, valid skills.
    #[test]
    fn val_discovery_009_disable_model_invocation_parsing() {
        let root = TempDir::new().unwrap();
        write_skill(
            root.path(),
            "on",
            "---\ndescription: d\ndisable-model-invocation: true\n---\nb",
        );
        write_skill(
            root.path(),
            "off",
            "---\ndescription: d\ndisable-model-invocation: false\n---\nb",
        );
        write_skill(root.path(), "absent", &skill_md("d", "b"));
        let bad_path = write_skill(
            root.path(),
            "wrong-type",
            "---\ndescription: d\ndisable-model-invocation: maybe\n---\nb",
        );

        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        let by: std::collections::HashMap<_, _> =
            skills.iter().map(|s| (s.name.as_str(), s)).collect();
        assert_eq!(by.len(), 3, "three good skills expected: {skills:?}");
        assert!(by["on"].disable_model_invocation);
        assert!(!by["off"].disable_model_invocation);
        assert!(!by["absent"].disable_model_invocation);

        assert_eq!(errors.len(), 1, "expected one loader error: {errors:?}");
        match &errors[0] {
            SkillError::Loader { path, source } => {
                assert_eq!(path, &bad_path);
                assert!(matches!(source, FrontmatterError::InvalidYaml(_)));
            }
            other => panic!("expected Loader error, got {other:?}"),
        }
    }

    // A scope root that is a regular file surfaces an Io error without panicking;
    // the other scopes still resolve.
    #[test]
    fn val_discovery_010_root_is_a_file_yields_io_error() {
        let real_root = TempDir::new().unwrap();
        write_skill(real_root.path(), "ok", &skill_md("d", "b"));
        let file_root = TempDir::new().unwrap();
        let file_path = file_root.path().join("not-a-dir");
        fs::write(&file_path, "i am a file").unwrap();

        let (skills, errors) = discover_skills(&[
            (file_path.clone(), SourceScope::AppData),
            (real_root.path().to_path_buf(), SourceScope::Project),
        ]);
        assert_eq!(
            skills.len(),
            1,
            "good scope should still resolve: {skills:?}"
        );
        assert_eq!(skills[0].name, "ok");
        assert_eq!(errors.len(), 1, "expected one io error: {errors:?}");
        match &errors[0] {
            SkillError::Io { path, .. } => assert_eq!(path, &file_path),
            other => panic!("expected Io error, got {other:?}"),
        }
    }

    // An unreadable SKILL.md surfaces an Io error rather than panicking.
    // Unix-only: permission bits are not portable.
    #[cfg(unix)]
    #[test]
    fn val_discovery_010_unreadable_skill_md_yields_io_error() {
        use std::os::unix::fs::PermissionsExt;

        // Root bypasses permission checks, so the assertion is meaningless there.
        if unsafe { libc_geteuid() } == 0 {
            return;
        }

        let root = TempDir::new().unwrap();
        let file = write_skill(root.path(), "locked", &skill_md("d", "b"));
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&file, perms).unwrap();

        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        assert!(
            skills.is_empty(),
            "unreadable skill should not load: {skills:?}"
        );
        assert_eq!(errors.len(), 1, "expected one io error: {errors:?}");
        assert!(
            matches!(errors[0], SkillError::Io { .. }),
            "expected Io error, got {:?}",
            errors[0]
        );

        // Restore permissions so the tempdir can be cleaned up.
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file, perms).unwrap();
    }

    // Minimal libc `geteuid` shim so the permission test can skip under root
    // without pulling in the `libc` crate as a dependency.
    #[cfg(unix)]
    extern "C" {
        #[link_name = "geteuid"]
        fn libc_geteuid() -> u32;
    }

    #[test]
    fn discovery_is_lenient_one_bad_does_not_abort() {
        let root = TempDir::new().unwrap();
        write_skill(root.path(), "good-a", &skill_md("a", "a"));
        write_skill(root.path(), "good-b", &skill_md("b", "b"));
        write_skill(root.path(), "broken", "---\nname: : :\n---\nbody");
        write_skill(root.path(), "no-desc", "---\nname: no-desc\n---\nbody");

        let (skills, errors) =
            discover_skills(&[(root.path().to_path_buf(), SourceScope::Project)]);
        let names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["good-a", "good-b"]);
        // One Loader (broken yaml) + one MissingDescription.
        assert_eq!(errors.len(), 2, "expected two diagnostics: {errors:?}");
    }

    /// Build a `Skill` directly (bypassing validation) for prompt-formatting
    /// tests. The path is set but must never appear in the formatted output.
    fn skill(name: &str, description: &str, disable: bool) -> Skill {
        Skill {
            name: name.to_string(),
            description: description.to_string(),
            body: "ignored body".to_string(),
            disable_model_invocation: disable,
            source: SourceInfo {
                scope: SourceScope::Project,
                path: PathBuf::from(format!("/abs/secret/path/{name}/SKILL.md")),
            },
        }
    }

    #[test]
    fn val_prompt_002_empty_list_is_none() {
        assert_eq!(format_skills_section(&[]), None);
    }

    #[test]
    fn val_prompt_001_non_empty_emits_name_and_description() {
        let out = format_skills_section(&[skill("alpha", "Alpha does things.", false)]).unwrap();
        assert!(
            out.contains("<available_skills>"),
            "missing wrapper:\n{out}"
        );
        assert!(out.contains("</available_skills>"), "missing close:\n{out}");
        assert!(out.contains("<name>alpha</name>"), "missing name:\n{out}");
        assert!(
            out.contains("<description>Alpha does things.</description>"),
            "missing description:\n{out}"
        );
        // Exact minimal block shape for a single, auto-invocable skill.
        assert!(
            out.contains("  <skill>\n    <name>alpha</name>\n    <description>Alpha does things.</description>\n  </skill>\n"),
            "unexpected single-skill block:\n{out}"
        );
    }

    // Alphabetical by name even when the input arrives in another order.
    #[test]
    fn val_prompt_003_sorted_by_name() {
        let out = format_skills_section(&[
            skill("zebra", "z", false),
            skill("apple", "a", false),
            skill("mango", "m", false),
        ])
        .unwrap();
        let pos_apple = out.find("<name>apple</name>").expect("apple");
        let pos_mango = out.find("<name>mango</name>").expect("mango");
        let pos_zebra = out.find("<name>zebra</name>").expect("zebra");
        assert!(
            pos_apple < pos_mango && pos_mango < pos_zebra,
            "names not alphabetical:\n{out}"
        );
    }

    // disable_model_invocation → <skill opt-in="true">; otherwise a plain <skill>.
    #[test]
    fn val_prompt_004_opt_in_attribute() {
        let out = format_skills_section(&[
            skill("auto", "auto-invocable", false),
            skill("manual", "opt-in only", true),
        ])
        .unwrap();
        assert!(
            out.contains("  <skill opt-in=\"true\">\n    <name>manual</name>"),
            "missing opt-in block:\n{out}"
        );
        assert!(
            out.contains("  <skill>\n    <name>auto</name>"),
            "missing plain block:\n{out}"
        );
        assert!(
            !out.contains("<skill opt-in=\"true\">\n    <name>auto</name>"),
            "auto skill wrongly tagged opt-in:\n{out}"
        );
    }

    // All five XML metacharacters are escaped in both <name> and <description>,
    // and an `&` is never double-escaped.
    #[test]
    fn val_prompt_005_special_characters_escaped() {
        let s = skill(
            "a&b<c",
            "quotes \" and ' and < and > and & and &amp;",
            false,
        );
        let out = format_skills_section(&[s]).unwrap();

        assert!(out.contains("<name>a&amp;b&lt;c</name>"), "name:\n{out}");

        // The literal "&amp;" input becomes "&amp;amp;": only the leading `&`
        // is escaped, so an already-emitted entity is not escaped twice.
        assert!(
            out.contains(
                "<description>quotes &quot; and &apos; and &lt; and &gt; and &amp; and &amp;amp;</description>"
            ),
            "description escaping:\n{out}"
        );
        assert!(!out.contains("a&b<c"), "raw name leaked:\n{out}");
    }

    #[test]
    fn val_prompt_005_escape_xml_unit() {
        assert_eq!(escape_xml("&"), "&amp;");
        assert_eq!(escape_xml("<"), "&lt;");
        assert_eq!(escape_xml(">"), "&gt;");
        assert_eq!(escape_xml("\""), "&quot;");
        assert_eq!(escape_xml("'"), "&apos;");
        // No double-escaping: an existing entity's `&` is escaped once.
        assert_eq!(escape_xml("&amp;"), "&amp;amp;");
        assert_eq!(escape_xml("a<b>c"), "a&lt;b&gt;c");
    }

    // The prose must say "call the skill tool" and never "read tool to load".
    #[test]
    fn val_prompt_006_guidance_prose() {
        let out = format_skills_section(&[skill("alpha", "a", false)]).unwrap();
        assert!(
            out.contains("call the skill tool"),
            "missing 'call the skill tool':\n{out}"
        );
        assert!(
            !out.contains("read tool to load"),
            "must not contain 'read tool to load':\n{out}"
        );
    }

    #[test]
    fn val_prompt_007_no_location_or_path() {
        let out = format_skills_section(&[skill("alpha", "a", false)]).unwrap();
        assert!(
            !out.contains("<location>"),
            "location element present:\n{out}"
        );
        assert!(
            !out.contains("/abs/secret/path/"),
            "absolute path leaked:\n{out}"
        );
        assert!(!out.contains("SKILL.md"), "SKILL.md path leaked:\n{out}");
    }

    // escape_xml does not touch '\n', so newlines survive inside <description>.
    #[test]
    fn val_prompt_008_multiline_description_preserved() {
        let desc = "line one\nline two\nline three";
        let out = format_skills_section(&[skill("alpha", desc, false)]).unwrap();
        assert!(
            out.contains("<description>line one\nline two\nline three</description>"),
            "newlines not preserved:\n{out}"
        );
        assert!(
            !out.contains("&#"),
            "newline wrongly entity-encoded:\n{out}"
        );
    }
}
