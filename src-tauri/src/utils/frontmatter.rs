//! YAML frontmatter parser for markdown content.
//!
//! Parses the `---\n<yaml>\n---\n<body>` envelope at the top of a markdown
//! string. The frontmatter block is OPTIONAL — sources without a leading
//! `---\n` line have no metadata and the body is the entire input.
//!
//! Used by skills (`SKILL.md`), prompt templates, and extension manifests.

use serde::de::DeserializeOwned;
use thiserror::Error;

/// Error returned when frontmatter parsing fails.
#[derive(Debug, Error)]
pub enum FrontmatterError {
    /// The leading `---\n` opener was found but the closing `---\n` line was missing.
    #[error("frontmatter opened with `---` but never closed")]
    UnterminatedFrontmatter,
    /// The YAML payload was malformed.
    #[error("invalid YAML in frontmatter: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),
}

/// Parsed frontmatter result. `metadata` is `None` when the input has no
/// leading frontmatter block.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFrontmatter<T> {
    pub metadata: Option<T>,
    pub body: String,
}

/// Parse markdown content, splitting frontmatter from body.
///
/// Frontmatter is recognized only when the input starts with `---\n` (or
/// `---\r\n`). Anything else is treated as body-only content.
///
/// `T` is the user's metadata type — a serde-deserializable struct or
/// `serde_yaml::Value` for dynamic schema.
pub fn parse_frontmatter<T: DeserializeOwned>(
    input: &str,
) -> Result<ParsedFrontmatter<T>, FrontmatterError> {
    // Strip a leading UTF-8 BOM if present. Editors on Windows occasionally
    // save markdown with a BOM; without this, `"\u{FEFF}---\n..."` would
    // miss the `---\n` opener and the whole file would be treated as body.
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);

    // Frontmatter is only recognized when the input starts with `---` followed
    // by a newline. A bare `---` with no newline (e.g. `"---name: foo---"`)
    // is treated as body-only content.
    let after_opener = if let Some(rest) = input.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = input.strip_prefix("---\r\n") {
        rest
    } else {
        return Ok(ParsedFrontmatter {
            metadata: None,
            body: input.to_string(),
        });
    };

    // Locate the closing `---` line. It must appear at the start of a line —
    // either right after a `\n` or at the very start of `after_opener` (the
    // empty-frontmatter case `"---\n---\n..."`).
    let (yaml_text, body) = match find_closer(after_opener) {
        Some((yaml_end, body_start)) => (&after_opener[..yaml_end], &after_opener[body_start..]),
        None => return Err(FrontmatterError::UnterminatedFrontmatter),
    };

    let metadata: T = serde_yaml::from_str(yaml_text)?;
    Ok(ParsedFrontmatter {
        metadata: Some(metadata),
        body: body.to_string(),
    })
}

/// Locate the closing `---` line within `after_opener`.
///
/// Returns `(yaml_end, body_start)` where:
/// - `after_opener[..yaml_end]` is the YAML payload (excluding the closing line),
/// - `after_opener[body_start..]` is the body (everything after the closing line's newline,
///   or empty if the closer is the last line of the input).
///
/// The closer is a line consisting solely of `---` (with optional trailing `\r`).
fn find_closer(after_opener: &str) -> Option<(usize, usize)> {
    let bytes = after_opener.as_bytes();
    let mut line_start = 0usize;

    loop {
        // Find end of current line.
        let line_end = bytes[line_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| line_start + i)
            .unwrap_or(bytes.len());

        // Strip a trailing \r if present (CRLF input).
        let content_end = if line_end > line_start && bytes[line_end - 1] == b'\r' {
            line_end - 1
        } else {
            line_end
        };

        if &bytes[line_start..content_end] == b"---" {
            // YAML payload ends at the byte before this line's leading newline.
            // For the very first line (line_start == 0) the YAML is empty.
            let yaml_end = line_start.saturating_sub(1);
            // Body starts after this line's terminating `\n` (if any).
            let body_start = if line_end < bytes.len() {
                line_end + 1
            } else {
                bytes.len()
            };
            return Some((yaml_end, body_start));
        }

        if line_end >= bytes.len() {
            return None;
        }
        line_start = line_end + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Default)]
    struct TestMeta {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
    }

    // 1. Basic happy path.
    #[test]
    fn parses_basic_frontmatter() {
        let input = "---\nname: foo\n---\nhello";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "hello");
    }

    // 2. No frontmatter — body-only.
    #[test]
    fn no_frontmatter_returns_body_only() {
        let input = "hello world";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert!(parsed.metadata.is_none());
        assert_eq!(parsed.body, "hello world");
    }

    // 3. Empty input.
    #[test]
    fn empty_input() {
        let parsed = parse_frontmatter::<TestMeta>("").unwrap();
        assert!(parsed.metadata.is_none());
        assert_eq!(parsed.body, "");
    }

    // 4. Frontmatter only, no body (closer followed by trailing newline).
    #[test]
    fn frontmatter_only_with_trailing_newline() {
        let input = "---\nname: foo\n---\n";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "");
    }

    // 5. Frontmatter only, no trailing newline after closer.
    #[test]
    fn frontmatter_only_no_trailing_newline() {
        let input = "---\nname: foo\n---";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "");
    }

    // 6. CRLF line endings.
    #[test]
    fn crlf_line_endings() {
        let input = "---\r\nname: foo\r\n---\r\nhello";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "hello");
    }

    // 7. Multi-line description (literal block `|`).
    #[test]
    fn multiline_literal_block_scalar() {
        let input = "---\ndescription: |\n  line one\n  line two\n---\nbody";
        let parsed = parse_frontmatter::<serde_yaml::Value>(input).unwrap();
        let meta = parsed.metadata.as_ref().unwrap();
        // serde_yaml applies "clip" chomping: collapses trailing newlines down
        // to none for the deserialized string (differs from the JS `yaml` lib
        // which preserves a single trailing `\n`). The interior `\n` separator
        // is what we care about.
        assert_eq!(meta["description"].as_str(), Some("line one\nline two"),);
        assert_eq!(parsed.body, "body");
    }

    // 8. Multi-line description (folded `>`).
    #[test]
    fn multiline_folded_block_scalar() {
        let input = "---\ndescription: >\n  line one\n  line two\n---\nbody";
        let parsed = parse_frontmatter::<serde_yaml::Value>(input).unwrap();
        let meta = parsed.metadata.as_ref().unwrap();
        // Folded scalar joins continuation lines with a single space; trailing
        // newline is clipped by serde_yaml.
        assert_eq!(meta["description"].as_str(), Some("line one line two"),);
        assert_eq!(parsed.body, "body");
    }

    // 9. Quoted string with special chars (colons inside quotes).
    #[test]
    fn quoted_string_with_colons() {
        let input = "---\ndescription: \"name: with: colons\"\n---\nbody";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().description.as_deref(),
            Some("name: with: colons"),
        );
        assert_eq!(parsed.body, "body");
    }

    // 10. Empty frontmatter block.
    #[test]
    fn empty_frontmatter_block() {
        let input = "---\n---\nbody";
        // Empty YAML deserializes to a unit/null. For a struct with all-default
        // optional fields, this round-trips to the default struct via
        // `serde_yaml::Value::Null` -> Option fields stay None.
        let parsed = parse_frontmatter::<serde_yaml::Value>(input).unwrap();
        // Empty YAML payload deserializes to `Value::Null`.
        assert!(parsed.metadata.as_ref().unwrap().is_null());
        assert_eq!(parsed.body, "body");
    }

    // 11. Unterminated frontmatter.
    #[test]
    fn unterminated_frontmatter_errors() {
        let input = "---\nname: foo\nno closer";
        let err = parse_frontmatter::<TestMeta>(input).unwrap_err();
        assert!(matches!(err, FrontmatterError::UnterminatedFrontmatter));
    }

    // 11b. Only the opener line, nothing else — still unterminated.
    #[test]
    fn opener_only_is_unterminated() {
        let err = parse_frontmatter::<TestMeta>("---\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::UnterminatedFrontmatter));
        // CRLF opener-only variant.
        let err = parse_frontmatter::<TestMeta>("---\r\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::UnterminatedFrontmatter));
    }

    // 12. Invalid YAML.
    #[test]
    fn invalid_yaml_errors() {
        let input = "---\nname: : :\n---\nbody";
        let err = parse_frontmatter::<TestMeta>(input).unwrap_err();
        assert!(matches!(err, FrontmatterError::InvalidYaml(_)));
    }

    // 13. Body contains `---` (not at line start of metadata).
    #[test]
    fn body_contains_triple_dash() {
        let input = "---\nname: foo\n---\nseparator: ---\nstill body";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "separator: ---\nstill body");
    }

    // 13b. A `---` line later in the body must NOT act as a second closer —
    // the first line-leading `---` is the real closer and everything after it
    // is body verbatim, including a standalone `---` line.
    #[test]
    fn body_triple_dash_line_does_not_reclose() {
        let input = "---\nname: foo\n---\nintro\n---\noutro";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "intro\n---\noutro");
    }

    // 14. Body has leading whitespace — preserve leading newlines.
    #[test]
    fn body_preserves_leading_newlines() {
        let input = "---\nname: foo\n---\n\n\nhello";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "\n\nhello");
    }

    // 15. Frontmatter is just `---\n---` (no body, empty payload).
    #[test]
    fn just_open_and_close() {
        let input = "---\n---";
        let parsed = parse_frontmatter::<serde_yaml::Value>(input).unwrap();
        // Empty YAML payload — deserializes to `Value::Null`. The metadata
        // is `Some(Null)` (frontmatter block was present, just empty).
        assert!(parsed.metadata.as_ref().unwrap().is_null());
        assert_eq!(parsed.body, "");
    }

    // 16. No leading newline — bare `---` is not a frontmatter opener.
    #[test]
    fn single_line_no_newlines_is_body() {
        let input = "---name: foo---";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert!(parsed.metadata.is_none());
        assert_eq!(parsed.body, "---name: foo---");
    }

    // 17. Comments in YAML are ignored by the parser.
    #[test]
    fn yaml_comments_ignored() {
        let input = "---\n# comment\nname: foo\n---\nbody";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "body");
    }

    // Bonus: comment-only frontmatter — yields Null payload, not a struct.
    #[test]
    fn comment_only_frontmatter_yields_null() {
        let input = "---\n# just a comment\n---\nbody";
        let parsed = parse_frontmatter::<serde_yaml::Value>(input).unwrap();
        assert!(parsed.metadata.as_ref().unwrap().is_null());
        assert_eq!(parsed.body, "body");
    }

    // Empty-block vs no-block must be two DIFFERENT results: an empty block is
    // `Some(Null)`, a missing block is `None`.
    #[test]
    fn empty_block_is_some_null_not_none() {
        let with_block = parse_frontmatter::<serde_yaml::Value>("---\n---\nbody").unwrap();
        let without_block = parse_frontmatter::<serde_yaml::Value>("body").unwrap();
        assert!(with_block.metadata.is_some());
        assert!(with_block.metadata.as_ref().unwrap().is_null());
        assert!(without_block.metadata.is_none());
    }

    // BOM handling: a UTF-8 BOM at the very start of the input must not
    // prevent frontmatter recognition.
    #[test]
    fn bom_prefixed_frontmatter_parses() {
        let input = "\u{FEFF}---\nname: foo\n---\nhello";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo"),
        );
        assert_eq!(parsed.body, "hello");
    }

    // BOM followed by CRLF opener.
    #[test]
    fn bom_prefixed_crlf_frontmatter_parses() {
        let input = "\u{FEFF}---\r\nname: foo\r\n---\r\nhello";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "hello");
    }

    // BOM followed by plain body (no frontmatter): the BOM is stripped and the
    // remaining text is the whole body.
    #[test]
    fn bom_prefixed_plain_text_is_body() {
        let input = "\u{FEFF}hello world";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert!(parsed.metadata.is_none());
        assert_eq!(parsed.body, "hello world");
    }

    // Bonus: CRLF closer immediately followed by EOF.
    #[test]
    fn crlf_closer_at_eof() {
        let input = "---\r\nname: foo\r\n---\r\n";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "");
    }

    // CRLF closer at EOF with no trailing newline (`...---` ends the input).
    #[test]
    fn crlf_closer_at_eof_no_trailing_newline() {
        let input = "---\r\nname: foo\r\n---";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "");
    }

    // Unknown frontmatter fields are tolerated (no `deny_unknown_fields`).
    #[test]
    fn unknown_fields_are_tolerated() {
        let input = "---\nname: foo\nunknown_field: 42\nanother: [1, 2, 3]\n---\nbody";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert_eq!(parsed.body, "body");
    }

    // Malformed input must return a deterministic Ok/Err, never panic.
    // 1) Truncated multi-byte UTF-8 cannot exist in a &str, so we exercise the
    //    closest analogue: a body whose bytes include multi-byte chars right at
    //    the closer boundary, plus a very long single line.
    #[test]
    fn multibyte_and_long_lines_do_not_panic() {
        // Multi-byte chars in YAML and body around the closer.
        let input = "---\nname: \"héllo — 世界\"\n---\n你好，world 🌍";
        let parsed = parse_frontmatter::<TestMeta>(input).unwrap();
        assert_eq!(
            parsed.metadata.as_ref().unwrap().name.as_deref(),
            Some("héllo — 世界")
        );
        assert_eq!(parsed.body, "你好，world 🌍");

        // A pathologically long single line as body — must not panic.
        let long_body: String = "x".repeat(200_000);
        let input = format!("---\nname: foo\n---\n{long_body}");
        let parsed = parse_frontmatter::<TestMeta>(&input).unwrap();
        assert_eq!(parsed.body.len(), 200_000);

        // A long single line that never closes — deterministic Err, no panic.
        let long_open = format!("---\n{}", "y".repeat(200_000));
        let err = parse_frontmatter::<TestMeta>(&long_open).unwrap_err();
        assert!(matches!(err, FrontmatterError::UnterminatedFrontmatter));
    }
}
