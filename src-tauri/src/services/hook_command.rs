//! Runs a hook rule's command and interprets what it says.
//!
//! The protocol follows Claude Code's: the event arrives as JSON on stdin, and
//! the command answers on stdout and through its exit code. That shape is what
//! makes a hook able to *do* something — format the file that was just written,
//! commit, notify — rather than only vote on whether a call may proceed.
//!
//! ```text
//! stdin   {"event":"before_tool_call","toolName":"bash","arguments":{...},...}
//! stdout  {"decision":"deny","reason":"..."}   → block the call
//!         {"updatedInput":{...}}               → run the tool with these args
//!         anything else / nothing              → proceed unchanged
//! exit    0        → stdout decides
//!         non-zero → block, stderr is the reason
//! ```
//!
//! Timeouts fail CLOSED on a pending call, matching the approval gate: a hook
//! that was asked for an opinion and did not answer is not consent.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::AsyncWriteExt;

/// Cap on captured stdout/stderr. A runaway hook must not pull its whole output
/// into memory; the decision JSON is tiny and anything past this is diagnostics.
const MAX_CAPTURED_OUTPUT: usize = 64 * 1024;

/// What the command told us to do.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandVerdict {
    /// Proceed unchanged.
    Proceed,
    /// Block the call with this reason.
    Deny(String),
    /// Run the tool with these arguments instead.
    ReplaceInput(serde_json::Value),
}

/// The subset of the response we act on. Unknown fields are ignored so a hook
/// can print extra diagnostics alongside its decision.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    updated_input: Option<serde_json::Value>,
}

/// Everything the command needs to run: what to run, where, and for how long.
pub struct CommandSpec<'a> {
    pub command: &'a str,
    pub working_dir: &'a Path,
    pub timeout: Duration,
    /// Extra environment on top of the inherited one. Used for the
    /// `HANDBOX_*` event variables so a script can branch without parsing JSON.
    pub env: Vec<(String, String)>,
}

/// Run the command, feed it `event`, and interpret the result.
///
/// Never returns an error: a hook that cannot be spawned, times out, or exits
/// non-zero produces a [`CommandVerdict::Deny`] with the reason. The caller
/// decides whether a deny is binding — after a tool call it no longer is.
pub async fn run_hook_command(spec: CommandSpec<'_>, event: &serde_json::Value) -> CommandVerdict {
    let payload = event.to_string();

    // `sh -c` because the field holds a command LINE — pipes and redirects are
    // exactly what makes a one-line hook useful.
    let mut cmd = tokio::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(spec.command)
        .current_dir(resolve_dir(spec.working_dir))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &spec.env {
        cmd.env(key, value);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return CommandVerdict::Deny(format!("hook command failed to start: {e}")),
    };

    if let Some(mut stdin) = child.stdin.take() {
        // A hook that ignores stdin closes it early; that is not an error, so
        // the broken-pipe case is swallowed rather than treated as a refusal.
        let _ = stdin.write_all(payload.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let output = match tokio::time::timeout(spec.timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return CommandVerdict::Deny(format!("hook command failed: {e}")),
        Err(_) => {
            return CommandVerdict::Deny(format!(
                "hook command timed out after {}ms",
                spec.timeout.as_millis()
            ))
        }
    };

    let stdout = truncate(&output.stdout);
    let stderr = truncate(&output.stderr);

    if !output.status.success() {
        // stderr is the conventional place for the reason; fall back to
        // something identifiable so the model never sees an empty refusal.
        let reason = first_non_empty(&[&stderr, &stdout])
            .unwrap_or_else(|| "hook command exited non-zero".to_string());
        return CommandVerdict::Deny(reason);
    }

    parse_verdict(&stdout)
}

/// Interpret stdout. Anything that is not a recognised decision object means
/// "no opinion" — a hook that just logs should not have to print JSON.
fn parse_verdict(stdout: &str) -> CommandVerdict {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return CommandVerdict::Proceed;
    }

    let Ok(response) = serde_json::from_str::<CommandResponse>(trimmed) else {
        return CommandVerdict::Proceed;
    };

    if response.decision.as_deref() == Some("deny") {
        return CommandVerdict::Deny(
            response
                .reason
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| "blocked by hook command".to_string()),
        );
    }

    match response.updated_input {
        Some(input) if input.is_object() => CommandVerdict::ReplaceInput(input),
        _ => CommandVerdict::Proceed,
    }
}

/// A session with no workspace has a cwd that may not exist; fall back rather
/// than failing the spawn for a reason the user cannot act on.
fn resolve_dir(dir: &Path) -> PathBuf {
    if dir.is_dir() {
        dir.to_path_buf()
    } else {
        std::env::temp_dir()
    }
}

fn truncate(bytes: &[u8]) -> String {
    let slice = if bytes.len() > MAX_CAPTURED_OUTPUT {
        &bytes[..MAX_CAPTURED_OUTPUT]
    } else {
        bytes
    };
    String::from_utf8_lossy(slice).to_string()
}

fn first_non_empty(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .map(|c| c.trim())
        .find(|c| !c.is_empty())
        .map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn spec<'a>(command: &'a str, dir: &'a Path) -> CommandSpec<'a> {
        CommandSpec {
            command,
            working_dir: dir,
            timeout: Duration::from_secs(5),
            env: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_silent_command_proceeds() {
        let dir = TempDir::new().unwrap();
        let verdict = run_hook_command(spec("true", dir.path()), &json!({})).await;
        assert_eq!(verdict, CommandVerdict::Proceed);
    }

    /// The plain side-effect case: a hook that just does work and says nothing.
    #[tokio::test]
    async fn a_command_runs_for_its_side_effect() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("ran.txt");
        let command = format!("echo done > {}", marker.display());

        let verdict = run_hook_command(spec(&command, dir.path()), &json!({})).await;

        assert_eq!(verdict, CommandVerdict::Proceed);
        assert!(marker.exists(), "the command's side effect must happen");
    }

    /// The event is on stdin, so a hook can branch on what is actually happening.
    #[tokio::test]
    async fn the_event_reaches_the_command_on_stdin() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("seen.json");
        let command = format!("cat > {}", out.display());

        run_hook_command(
            spec(&command, dir.path()),
            &json!({"event": "before_tool_call", "toolName": "bash"}),
        )
        .await;

        let seen: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(seen["toolName"], "bash");
        assert_eq!(seen["event"], "before_tool_call");
    }

    #[tokio::test]
    async fn a_deny_decision_blocks_with_its_reason() {
        let dir = TempDir::new().unwrap();
        let command = r#"echo '{"decision":"deny","reason":"not on main"}'"#;

        let verdict = run_hook_command(spec(command, dir.path()), &json!({})).await;

        assert_eq!(verdict, CommandVerdict::Deny("not on main".to_string()));
    }

    #[tokio::test]
    async fn updated_input_rewrites_the_arguments() {
        let dir = TempDir::new().unwrap();
        let command = r#"echo '{"updatedInput":{"command":"ls -la"}}'"#;

        let verdict = run_hook_command(spec(command, dir.path()), &json!({})).await;

        assert_eq!(
            verdict,
            CommandVerdict::ReplaceInput(json!({"command": "ls -la"}))
        );
    }

    /// Exit code is the low-ceremony way to block, so a one-liner needs no JSON.
    #[tokio::test]
    async fn a_non_zero_exit_blocks_with_stderr() {
        let dir = TempDir::new().unwrap();
        let command = "echo 'refused by policy' >&2; exit 1";

        let verdict = run_hook_command(spec(command, dir.path()), &json!({})).await;

        match verdict {
            CommandVerdict::Deny(reason) => assert!(
                reason.contains("refused by policy"),
                "stderr should carry the reason, got: {reason}"
            ),
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    /// A hook that logs prose must not be mistaken for one that has an opinion.
    #[tokio::test]
    async fn non_json_output_is_not_a_decision() {
        let dir = TempDir::new().unwrap();
        let verdict = run_hook_command(spec("echo formatting done", dir.path()), &json!({})).await;
        assert_eq!(verdict, CommandVerdict::Proceed);
    }

    /// Fail closed: a hook asked for an opinion that never answers is not consent.
    #[tokio::test]
    async fn a_timeout_denies() {
        let dir = TempDir::new().unwrap();
        let mut s = spec("sleep 5", dir.path());
        s.timeout = Duration::from_millis(150);

        let verdict = run_hook_command(s, &json!({})).await;

        match verdict {
            CommandVerdict::Deny(reason) => {
                assert!(reason.contains("timed out"), "got: {reason}")
            }
            other => panic!("expected Deny on timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_command_that_cannot_run_denies() {
        let dir = TempDir::new().unwrap();
        let verdict = run_hook_command(
            spec("definitely-not-a-real-binary-xyz", dir.path()),
            &json!({}),
        )
        .await;
        assert!(matches!(verdict, CommandVerdict::Deny(_)));
    }

    /// Env vars let a script branch without parsing the JSON at all.
    #[tokio::test]
    async fn env_variables_are_available_to_the_command() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("env.txt");
        let command = format!("printf '%s' \"$HANDBOX_TOOL_NAME\" > {}", out.display());

        let mut s = spec(&command, dir.path());
        s.env = vec![("HANDBOX_TOOL_NAME".to_string(), "bash".to_string())];
        run_hook_command(s, &json!({})).await;

        assert_eq!(std::fs::read_to_string(&out).unwrap(), "bash");
    }

    /// A workspace-less session has a cwd that may not exist; the hook should
    /// still run rather than failing for a reason the user cannot act on.
    #[tokio::test]
    async fn a_missing_working_dir_falls_back_instead_of_failing() {
        let missing = PathBuf::from("/definitely/not/here/at/all");
        let verdict = run_hook_command(spec("true", &missing), &json!({})).await;
        assert_eq!(verdict, CommandVerdict::Proceed);
    }
}
