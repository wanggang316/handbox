//! Skills are markdown files describing optional capabilities the model can use.
//!
//! A skill is a directory containing a `SKILL.md` file. The file's YAML
//! frontmatter declares metadata (`name`, `description`,
//! `disable-model-invocation`); the body is the prose injected into the system
//! prompt's "Skills" section when the skill is enabled.
//!
//! This module owns the skill data model and validation only. Discovery
//! (filesystem traversal, dedup) lives in a separate module and feeds its
//! parsed results into [`validate`], which is filesystem-independent so it can
//! be unit-tested in isolation.

use crate::utils::frontmatter::FrontmatterError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Skill name length cap (bytes) per the Agent Skills spec.
const MAX_NAME_LENGTH: usize = 64;
/// Description length cap (bytes) per the Agent Skills spec.
const MAX_DESCRIPTION_LENGTH: usize = 1024;

/// The scope a skill was discovered in. Higher-priority scopes shadow
/// lower-priority scopes by canonical name during dedup.
///
/// The `Ord` derivation makes `Project` the greatest variant so a simple
/// `max`/sort picks the highest-priority source. The serde representation is
/// the contract for downstream IPC/UI: `"project"` / `"user"` / `"appData"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceScope {
    /// Application-bundled defaults — lowest priority.
    AppData,
    /// User-level skills, typically `~/.hand/skills/`.
    User,
    /// Project-level skills, typically `<cwd>/.hand/skills/` — highest priority.
    Project,
}

/// Where a skill was loaded from.
///
/// `path` points at the `SKILL.md` file or its directory. This module only
/// passes it through (for error location); the discovery module populates it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceInfo {
    pub scope: SourceScope,
    pub path: PathBuf,
}

/// Parsed YAML frontmatter on a `SKILL.md`.
///
/// Unknown fields are tolerated (no `deny_unknown_fields`) so skills can carry
/// forward-compatible metadata without tripping the loader. All fields default,
/// so an empty frontmatter block (`Value::Null`) deserializes into the default.
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
    /// Where the skill was discovered.
    pub source: SourceInfo,
}

/// Errors raised while loading and validating a `SKILL.md`.
///
/// Wraps loader-level errors (`Loader`) plus skill-specific schema errors.
/// Each variant carries the offending `path` for diagnostics.
#[derive(Debug, Error)]
pub enum SkillError {
    /// IO or frontmatter error from the underlying loader. Used by the
    /// discovery module; defined here so the error surface is complete.
    #[error("loader error in {path}: {source}")]
    Loader {
        path: PathBuf,
        #[source]
        source: FrontmatterError,
    },
    /// `SKILL.md` frontmatter is missing the required `description` field.
    #[error("missing required field `description` in {path}")]
    MissingDescription { path: PathBuf },
    /// `description` exceeds the spec-mandated length cap.
    #[error("description exceeds {max} bytes ({actual}) in {path}")]
    DescriptionTooLong {
        path: PathBuf,
        actual: usize,
        max: usize,
    },
    /// Frontmatter `name` doesn't match the directory name.
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
/// Inputs are already split from the `SKILL.md` envelope by the discovery
/// module: `dir_name` is the skill's directory basename, `metadata` is the
/// deserialized frontmatter (`None` when there was no frontmatter block; an
/// empty block deserializes to `Some(SkillMetadata::default())`), `body` is the
/// content after the frontmatter, and `source` records the origin.
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

    // If frontmatter `name` is provided, it must match the directory name.
    // Otherwise fall back to the directory name itself.
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

/// Validate a skill name per the Agent Skills spec.
///
/// Returns `Ok(())` if valid, `Err(reason)` otherwise. Length is measured in
/// bytes (`str::len`).
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

    // VAL-VALIDATION-001: a legal name passes (no frontmatter.name → dir fallback).
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

    // VAL-VALIDATION-001: assorted legal names all pass through the dir-fallback.
    #[test]
    fn val_001_valid_name_variants_pass() {
        for name in ["valid-name", "valid", "v123", "a", "a-b-c", "0-9"] {
            let res = run(name, Some(meta(None, Some("ok"))));
            assert!(res.is_ok(), "expected ok for {name:?}, got {res:?}");
            assert_eq!(res.unwrap().name, name);
        }
    }

    // VAL-VALIDATION-002: each name violation, via the dir-fallback path (no
    // frontmatter.name), yields InvalidName with a reason naming its category.
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

    // VAL-VALIDATION-002: oversized dir name (no frontmatter) → InvalidName with
    // the length-category reason (distinct from the >64 NameMismatch path).
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

    // VAL-VALIDATION-003: 64-byte name passes, 65-byte name fails (byte length).
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

    // VAL-VALIDATION-004: description missing → MissingDescription.
    #[test]
    fn val_004_missing_description() {
        match run("ok-skill", Some(meta(None, None))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // VAL-VALIDATION-004: whitespace-only description → MissingDescription.
    #[test]
    fn val_004_whitespace_only_description() {
        match run("ok-skill", Some(meta(None, Some("   \n\t  ")))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // VAL-VALIDATION-004: YAML null description (deserializes to None) →
    // MissingDescription. Simulated via SkillMetadata with description: None,
    // which is what `description: null` deserializes to.
    #[test]
    fn val_004_yaml_null_description() {
        let m: SkillMetadata = serde_yaml::from_str("name: ok-skill\ndescription: null\n").unwrap();
        assert!(m.description.is_none());
        match run("ok-skill", Some(m)) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // VAL-VALIDATION-005: 1024-byte description passes, 1025 fails. Name must be
    // valid for the length check to be reached.
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

    // VAL-VALIDATION-006: frontmatter.name != dir_name → NameMismatch.
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

    // VAL-VALIDATION-007: fixed order — description-required runs before
    // name checks. Missing description + name mismatch → MissingDescription.
    #[test]
    fn val_007_order_missing_description_beats_name_mismatch() {
        match run("dir-name", Some(meta(Some("other-name"), None))) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription first, got {other:?}"),
        }
    }

    // VAL-VALIDATION-007: description-length runs before name-mismatch.
    // 1025-byte description + name mismatch → DescriptionTooLong.
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

    // VAL-VALIDATION-007: name-mismatch runs before name-valid. A frontmatter
    // name that is *itself* invalid but differs from dir → NameMismatch, not
    // InvalidName (InvalidName is only reachable via the two name==dir paths).
    #[test]
    fn val_007_mismatch_beats_invalid_when_names_differ() {
        // frontmatter "Bad_Name" is invalid AND != dir "good-dir" → NameMismatch.
        match run("good-dir", Some(meta(Some("Bad_Name"), Some("ok")))) {
            Err(SkillError::NameMismatch { .. }) => {}
            other => panic!("expected NameMismatch, got {other:?}"),
        }
    }

    // VAL-VALIDATION-007: InvalidName path #2 — frontmatter.name == dir_name and
    // both are invalid. The mismatch check passes (equal), then name-valid fails.
    #[test]
    fn val_007_invalid_name_via_matching_invalid_frontmatter() {
        match run("Bad_Name", Some(meta(Some("Bad_Name"), Some("ok")))) {
            Err(SkillError::InvalidName { name, .. }) => {
                assert_eq!(name, "Bad_Name");
            }
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    // VAL-VALIDATION-008: no frontmatter (metadata None) → MissingDescription.
    #[test]
    fn val_008_no_frontmatter_missing_description() {
        match run("ok-skill", None) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // VAL-VALIDATION-008: empty YAML block — Value::Null deserializes to a
    // default SkillMetadata, which has no description → MissingDescription.
    #[test]
    fn val_008_empty_yaml_block_missing_description() {
        // `serde_yaml::from_value(Value::Null)` round-trips to the default
        // struct because all fields are #[serde(default)]. The discovery
        // feature passes exactly this `Some(default)` for an empty block.
        let from_null: SkillMetadata = serde_yaml::from_value(serde_yaml::Value::Null).unwrap();
        assert_eq!(from_null, SkillMetadata::default());
        match run("ok-skill", Some(from_null)) {
            Err(SkillError::MissingDescription { .. }) => {}
            other => panic!("expected MissingDescription, got {other:?}"),
        }
    }

    // VAL-VALIDATION-009: non-ASCII / dotted / slashed names via the
    // consistent path (frontmatter.name == dir_name) → InvalidName.
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

    // VAL-VALIDATION-010: description length uses the UNTRIMMED original. A
    // 1024-non-whitespace-byte body plus one trailing space (1025 bytes) is
    // TooLong even though its trimmed length is 1024.
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

    // VAL-VALIDATION-011: a legal description is preserved VERBATIM (not
    // trimmed) in the resulting Skill.
    #[test]
    fn val_011_description_preserved_verbatim() {
        let desc = "  surrounded by spaces  \nand a newline  ";
        let skill = run("ok-skill", Some(meta(None, Some(desc)))).unwrap();
        assert_eq!(skill.description, desc);
    }

    // SourceScope serde contract: variants serialize to the camelCase literals
    // that downstream IPC/UI depend on.
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
        // And round-trips back.
        let p: SourceScope = serde_json::from_str("\"project\"").unwrap();
        assert_eq!(p, SourceScope::Project);
    }

    // SourceScope priority order: Project is the greatest variant so dedup can
    // pick the highest-priority source with `max`/sort.
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

    // disable-model-invocation is parsed via its kebab-case serde rename.
    #[test]
    fn disable_model_invocation_kebab_rename_and_default() {
        let on: SkillMetadata =
            serde_yaml::from_str("description: x\ndisable-model-invocation: true\n").unwrap();
        assert!(on.disable_model_invocation);
        let skill = run("ok-skill", Some(on)).unwrap();
        assert!(skill.disable_model_invocation);

        // Absent → defaults to false.
        let off: SkillMetadata = serde_yaml::from_str("description: x\n").unwrap();
        assert!(!off.disable_model_invocation);
    }
}
