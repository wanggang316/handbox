//! ask_question — pause the turn and ask the user structured questions.
//!
//! INTERACTIVE tool: unlike the presentational `render_card` / `render_app`, the
//! handler does not just acknowledge — it emits [`QUESTION_REQUEST_EVENT`] and
//! parks on a oneshot until the frontend answers via the
//! `agent_question_respond` IPC, so the model's turn genuinely blocks on the
//! user. The round-trip mirrors `agent_permission`'s approval gate (process-level
//! `request_id → PendingQuestion` registry, first-response-wins, abort drops the
//! sender), but sits in the tool layer rather than a `before_tool_call` hook: the
//! answers ARE the tool result, so they must flow back to the model as content.
//!
//! FAIL-OPEN, not fail-closed: with no emitter (no UI to ask through) the tool
//! returns an error result telling the model to ask in plain prose instead.
//! Asking a question is not a privileged operation, so an unavailable surface
//! must degrade to conversation, never deny.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use hand_agent::{AgentTool, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;

/// Registration name the session's `enabled_tools` opts in with.
pub const TOOL_ASK_QUESTION: &str = "ask_question";

/// Tauri event the frontend listens on for a question request. Carries
/// `{ sessionId, callId, requestId, questions }` and is answered via the
/// `agent_question_respond` IPC.
pub const QUESTION_REQUEST_EVENT: &str = "agent_question_request";

/// Emitter pushing a question request to the frontend (wraps
/// `window.emit("agent_question_request", ..)`), keeping this module decoupled
/// from Tauri. `None` → no ask surface; see the module docs' fail-open rule.
pub type QuestionEmitter = Arc<dyn Fn(Value) + Send + Sync>;

/// Upper bound on questions per call. More than a handful in one panel is a
/// wall of forms, not a conversation — the model should ask, act, then ask again.
const MAX_QUESTIONS: usize = 4;
/// Upper bound on options per choice question, so the panel stays scannable.
const MAX_OPTIONS: usize = 8;
/// A choice question with a single option is a statement, not a question.
const MIN_OPTIONS: usize = 2;

/// How a question is answered. The snake_case wire values are the exact strings
/// the tool schema accepts and the frontend switches its renderer on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// Pick exactly one option.
    Single,
    /// Pick any number of options.
    Multiple,
    /// Free-form text.
    Text,
}

impl QuestionKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "single" => Some(Self::Single),
            "multiple" => Some(Self::Multiple),
            "text" => Some(Self::Text),
            _ => None,
        }
    }

    fn needs_options(self) -> bool {
        matches!(self, Self::Single | Self::Multiple)
    }
}

/// One selectable option of a choice question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    /// Optional one-line elaboration rendered under the label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A validated question, as emitted to the frontend. The parked tool call keeps
/// its own copy, so the RESULT text is composed from the model's own wording and
/// never from strings the frontend echoes back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// Stable per-call id (`q0`, `q1`, …) the answers are keyed by.
    pub id: String,
    /// Very short chip label.
    pub header: String,
    /// The question itself.
    pub question: String,
    #[serde(rename = "type")]
    pub kind: QuestionKind,
    /// Empty for [`QuestionKind::Text`].
    pub options: Vec<QuestionOption>,
    /// The panel blocks submission until this one is answered. Defaults to
    /// false: a question the model merely *wants* answered must not trap the
    /// user, so requiring an answer has to be an explicit decision per question.
    pub required: bool,
}

/// One question's answer as submitted by the frontend: the selected labels, or
/// a single free-text value. An omitted / empty entry reads as "not answered".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionAnswer {
    pub question_id: String,
    #[serde(default)]
    pub values: Vec<String>,
}

/// The user's response to one request. `Dismissed` is the "继续沟通" escape: a
/// deliberate refusal to answer, reported to the model as such rather than as a
/// failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuestionResponse {
    Answered { answers: Vec<QuestionAnswer> },
    Dismissed,
}

/// One pending request: the wake channel plus the session it belongs to, so
/// [`cancel_pending_questions_for_session`] — which only knows a session id — can
/// drop the right entries.
struct PendingQuestion {
    session_id: String,
    sender: oneshot::Sender<QuestionResponse>,
}

/// Process-level `request_id → PendingQuestion` registry. Process-level because
/// the tool closure is owned by the run's driver task while the stateless
/// `agent_question_respond` command must reach the same entries.
fn pending_questions() -> &'static Mutex<HashMap<String, PendingQuestion>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingQuestion>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve a pending question request.
///
/// IDEMPOTENT: the entry is removed before use, so the FIRST response for a
/// `request_id` wins and a duplicate or unknown id is a clean no-op — the
/// frontend may answer twice in a race, or answer a request an aborted run
/// already abandoned.
pub fn respond_to_question(request_id: &str, response: QuestionResponse) {
    let pending = pending_questions().lock().unwrap().remove(request_id);
    if let Some(pending) = pending {
        // The receiver may already be gone (run aborted) — nothing to wake.
        let _ = pending.sender.send(response);
    }
}

/// Drop EVERY pending question for `session_id`, unblocking the parked tool with
/// a cancellation.
///
/// The abort path: the tool awaits a BARE `rx.await` that does not race the run's
/// cancel token, so flipping the token alone cannot unblock a turn parked on a
/// question. Dropping the sender closes the oneshot and resolves that await.
pub fn cancel_pending_questions_for_session(session_id: &str) {
    let mut pending = pending_questions().lock().unwrap();
    // Collect first: we can't remove while iterating the borrowed map.
    let request_ids: Vec<String> = pending
        .iter()
        .filter(|(_, p)| p.session_id == session_id)
        .map(|(id, _)| id.clone())
        .collect();
    for request_id in request_ids {
        pending.remove(&request_id);
    }
}

/// Result text when the tool has no UI to ask through. An error (not plain
/// text) so the model treats the call as unavailable and falls back to prose.
const NO_SURFACE_MSG: &str =
    "no interactive surface is available to ask the user; ask your question in plain text instead";
/// Result text when the run was aborted while the panel was open.
const CANCELLED_MSG: &str = "the question was cancelled before the user answered";

/// Build the `ask_question` tool for one session.
///
/// `session_id` MUST be the HandBox session UUID the abort path calls
/// [`cancel_pending_questions_for_session`] with, or an abort cannot unblock a
/// parked question. `emitter: None` fails OPEN (see the module docs).
pub fn make_ask_question_tool(session_id: String, emitter: Option<QuestionEmitter>) -> AgentTool {
    AgentTool::simple(
        TOOL_ASK_QUESTION,
        "Ask the user one or more structured questions and wait for their \
         answer. The questions appear as a panel above the composer; the user \
         picks options or types a reply, and their answers come back as this \
         tool's result.\n\
         \n\
         Use it when the work genuinely forks on something only the user can \
         decide — a product or design choice, which of several viable \
         approaches to take, a missing constraint you would otherwise have to \
         guess. Do NOT use it for things you can determine yourself by reading \
         the code, for confirmation of a decision the user already made, or to \
         narrate progress. Prefer acting on a sensible default over asking; \
         when you do ask, ask everything you need in ONE call rather than \
         interrogating the user turn after turn.\n\
         \n\
         Each question declares its own `type`: `single` (pick one option), \
         `multiple` (pick any number) or `text` (free-form reply). Choice \
         questions need 2-8 concrete, mutually distinguishable options — write \
         real alternatives, never \"yes\"/\"no\" padding — and each option may \
         carry a one-line `description` spelling out its trade-off. `header` is \
         a 1-3 word chip label for the question, not a restatement of it.\n\
         \n\
         Mark a question `required: true` ONLY when you truly cannot proceed \
         without it — that blocks the submit button until it is answered. \
         Everything else stays optional, and a question the user leaves blank \
         comes back marked as unanswered.\n\
         \n\
         The user may dismiss the panel instead of answering: the result then \
         says so, and you must continue the conversation without the answer \
         rather than asking again.",
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_QUESTIONS,
                    "description": "The questions to ask, rendered in order in one panel.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "header": {
                                "type": "string",
                                "description": "Very short chip label for the question (1-3 words, e.g. \"Auth method\")."
                            },
                            "question": {
                                "type": "string",
                                "description": "The question itself, phrased as a complete sentence."
                            },
                            "type": {
                                "type": "string",
                                "enum": ["single", "multiple", "text"],
                                "description": "single = pick one option, multiple = pick any number, text = free-form reply."
                            },
                            "required": {
                                "type": "boolean",
                                "description": "Set true only when you genuinely cannot proceed without this answer — it blocks the submit button until the user answers. Defaults to false; prefer leaving it out."
                            },
                            "options": {
                                "type": "array",
                                "minItems": MIN_OPTIONS,
                                "maxItems": MAX_OPTIONS,
                                "description": "Required for single/multiple, omitted for text.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "The choice as the user reads it (concise)."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Optional one-line elaboration: what this choice implies or trades off."
                                        }
                                    },
                                    "required": ["label"]
                                }
                            }
                        },
                        "required": ["header", "question", "type"]
                    }
                }
            },
            "required": ["questions"]
        }),
        "Ask question",
        move |call_id, args| {
            let session_id = session_id.clone();
            let emitter = emitter.clone();
            async move { execute_ask_question(&session_id, emitter.as_ref(), &call_id, args).await }
        },
    )
}

/// `ask_question` body: validate, emit, park, then render the answer for the
/// model. Argument errors come back as `ToolResult::error` so the model can
/// correct the call itself.
async fn execute_ask_question(
    session_id: &str,
    emitter: Option<&QuestionEmitter>,
    call_id: &str,
    args: Value,
) -> ToolResult {
    let questions = match parse_questions(&args) {
        Ok(questions) => questions,
        Err(message) => return ToolResult::error(message),
    };

    // No UI to ask through → tell the model to ask in prose (fail-open).
    let Some(emitter) = emitter else {
        return ToolResult::error(NO_SURFACE_MSG);
    };

    // Register the wake channel BEFORE emitting, so a response racing back the
    // instant the event lands still finds a live entry to resolve.
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel::<QuestionResponse>();
    pending_questions().lock().unwrap().insert(
        request_id.clone(),
        PendingQuestion {
            session_id: session_id.to_string(),
            sender: tx,
        },
    );

    emitter(json!({
        "sessionId": session_id,
        "callId": call_id,
        "requestId": request_id,
        "questions": questions,
    }));

    match rx.await {
        Ok(QuestionResponse::Answered { answers }) => {
            ToolResult::text(format_answers(&questions, &answers))
        }
        Ok(QuestionResponse::Dismissed) => ToolResult::text(DISMISSED_RESULT),
        Err(_) => {
            // Run aborted: clean up any lingering entry and report cancellation.
            pending_questions().lock().unwrap().remove(&request_id);
            ToolResult::error(CANCELLED_MSG)
        }
    }
}

/// Result text for a dismissed panel. Must read as a deliberate user choice, not
/// a malfunction, so the model proceeds instead of retrying the tool.
const DISMISSED_RESULT: &str = "The user dismissed the questions without answering and wants to \
     keep talking instead. Do not ask again with this tool — continue the \
     conversation, and state the assumption you are proceeding with.";

/// Validate and normalize the `questions` argument. `Err` carries a message
/// written for the model to correct its own call.
fn parse_questions(args: &Value) -> Result<Vec<Question>, String> {
    let raw = args
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{TOOL_ASK_QUESTION} requires a `questions` array argument"))?;

    if raw.is_empty() {
        return Err("`questions` must contain at least one question".to_string());
    }
    if raw.len() > MAX_QUESTIONS {
        return Err(format!(
            "`questions` accepts at most {MAX_QUESTIONS} questions per call, got {}",
            raw.len()
        ));
    }

    raw.iter()
        .enumerate()
        .map(|(index, item)| parse_question(index, item))
        .collect()
}

/// Validate one question. `index` seeds the id and every error message, so the
/// model knows which entry to fix.
fn parse_question(index: usize, item: &Value) -> Result<Question, String> {
    let position = index + 1;
    let header = non_empty_str(item, "header")
        .ok_or_else(|| format!("question {position}: `header` must be a non-empty string"))?;
    let question = non_empty_str(item, "question")
        .ok_or_else(|| format!("question {position}: `question` must be a non-empty string"))?;
    let kind_raw = non_empty_str(item, "type")
        .ok_or_else(|| format!("question {position}: `type` must be a non-empty string"))?;
    let kind = QuestionKind::parse(kind_raw).ok_or_else(|| {
        format!("question {position}: `type` must be one of \"single\", \"multiple\", \"text\"")
    })?;

    let options = parse_options(position, item, kind)?;
    // Absent / non-boolean reads as optional: the permissive default keeps a
    // malformed flag from silently trapping the user in the panel.
    let required = item
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(Question {
        id: format!("q{index}"),
        header: header.to_string(),
        question: question.to_string(),
        kind,
        options,
        required,
    })
}

/// Options are required for the choice kinds and dropped for `text` — a text
/// question carrying options is a mistake worth reporting, not silently eating,
/// since the user would never see them.
fn parse_options(
    position: usize,
    item: &Value,
    kind: QuestionKind,
) -> Result<Vec<QuestionOption>, String> {
    let raw = item.get("options").and_then(Value::as_array);

    if !kind.needs_options() {
        return match raw {
            Some(options) if !options.is_empty() => Err(format!(
                "question {position}: a \"text\" question must not carry `options`"
            )),
            _ => Ok(Vec::new()),
        };
    }

    let raw = raw.ok_or_else(|| {
        format!(
            "question {position}: a \"single\"/\"multiple\" question requires an `options` array"
        )
    })?;
    if raw.len() < MIN_OPTIONS {
        return Err(format!(
            "question {position}: needs at least {MIN_OPTIONS} options, got {}",
            raw.len()
        ));
    }
    if raw.len() > MAX_OPTIONS {
        return Err(format!(
            "question {position}: accepts at most {MAX_OPTIONS} options, got {}",
            raw.len()
        ));
    }

    raw.iter()
        .enumerate()
        .map(|(option_index, option)| {
            let label = non_empty_str(option, "label").ok_or_else(|| {
                format!(
                    "question {position}, option {}: `label` must be a non-empty string",
                    option_index + 1
                )
            })?;
            Ok(QuestionOption {
                label: label.to_string(),
                description: non_empty_str(option, "description").map(str::to_string),
            })
        })
        .collect()
}

/// Trimmed string field, `None` when missing / non-string / blank.
fn non_empty_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Render the answers for the model, pairing each answer with the question text
/// the MODEL wrote (looked up by id) rather than anything the frontend echoed
/// back. A question with no values reads as explicitly unanswered, so a partial
/// submission is never mistaken for a full one.
fn format_answers(questions: &[Question], answers: &[QuestionAnswer]) -> String {
    let mut out = String::from("The user answered:\n");
    for (index, question) in questions.iter().enumerate() {
        let values: Vec<&str> = answers
            .iter()
            .find(|a| a.question_id == question.id)
            .map(|a| {
                a.values
                    .iter()
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        out.push_str(&format!("\n{}. {}\n", index + 1, question.question));
        if values.is_empty() {
            out.push_str("   (not answered)\n");
        } else {
            for value in values {
                out.push_str(&format!("   - {value}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::test_support::get_text;
    use super::*;

    fn single_question_args() -> Value {
        json!({
            "questions": [{
                "header": "Storage",
                "question": "Which storage backend should the cache use?",
                "type": "single",
                "options": [
                    {"label": "SQLite", "description": "Durable, one file"},
                    {"label": "In-memory"}
                ]
            }]
        })
    }

    #[test]
    fn parses_a_choice_question_into_a_normalized_shape() {
        let questions = parse_questions(&single_question_args()).expect("valid args parse");
        assert_eq!(questions.len(), 1);
        let q = &questions[0];
        // The id is positional so answers can key off it without the frontend
        // inventing identifiers.
        assert_eq!(q.id, "q0");
        assert_eq!(q.header, "Storage");
        assert_eq!(q.kind, QuestionKind::Single);
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.options[0].label, "SQLite");
        assert_eq!(
            q.options[0].description.as_deref(),
            Some("Durable, one file")
        );
        assert_eq!(q.options[1].description, None);
    }

    #[test]
    fn parses_multiple_questions_of_mixed_kinds_with_positional_ids() {
        let questions = parse_questions(&json!({
            "questions": [
                {
                    "header": "Targets",
                    "question": "Which platforms should ship first?",
                    "type": "multiple",
                    "options": [{"label": "macOS"}, {"label": "Windows"}, {"label": "Linux"}]
                },
                {
                    "header": "Name",
                    "question": "What should the feature be called?",
                    "type": "text"
                }
            ]
        }))
        .expect("mixed kinds parse");

        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].kind, QuestionKind::Multiple);
        assert_eq!(questions[1].kind, QuestionKind::Text);
        assert_eq!(questions[1].options, Vec::new());
        let ids: Vec<&str> = questions.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, vec!["q0", "q1"]);
    }

    #[test]
    fn required_defaults_to_false_and_is_read_when_present() {
        // Permissive default: only an explicit `true` may block the submit
        // button, so a missing or malformed flag can never trap the user.
        let questions = parse_questions(&json!({
            "questions": [
                {"header": "A", "question": "Optional?", "type": "text"},
                {"header": "B", "question": "Must?", "type": "text", "required": true},
                {"header": "C", "question": "Explicit no?", "type": "text", "required": false},
                {"header": "D", "question": "Bad flag?", "type": "text", "required": "yes"}
            ]
        }))
        .expect("required is optional");
        let flags: Vec<bool> = questions.iter().map(|q| q.required).collect();
        assert_eq!(flags, vec![false, true, false, false]);
    }

    #[test]
    fn required_is_serialized_to_the_panel() {
        let questions = parse_questions(&json!({
            "questions": [{"header": "B", "question": "Must?", "type": "text", "required": true}]
        }))
        .unwrap();
        let wire = serde_json::to_value(&questions[0]).unwrap();
        assert_eq!(wire.get("required").unwrap(), &json!(true));
    }

    #[test]
    fn missing_or_empty_questions_array_is_rejected() {
        assert!(parse_questions(&json!({})).is_err());
        assert!(parse_questions(&json!({"questions": "nope"})).is_err());
        assert!(parse_questions(&json!({"questions": []})).is_err());
    }

    #[test]
    fn too_many_questions_are_rejected() {
        let one = json!({
            "header": "H", "question": "Q?", "type": "text"
        });
        let raw: Vec<Value> = std::iter::repeat_n(one, MAX_QUESTIONS + 1).collect();
        let err = parse_questions(&json!({ "questions": raw })).expect_err("over the cap");
        assert!(
            err.contains(&MAX_QUESTIONS.to_string()),
            "the error must name the cap so the model can fix the call: {err:?}"
        );
    }

    #[test]
    fn blank_required_fields_are_rejected_per_question() {
        // Whitespace-only is as absent as missing: the panel would render blank.
        for bad in [
            json!({"header": "  ", "question": "Q?", "type": "text"}),
            json!({"header": "H", "question": "", "type": "text"}),
            json!({"header": "H", "question": "Q?"}),
        ] {
            assert!(
                parse_questions(&json!({ "questions": [bad] })).is_err(),
                "a blank required field must be rejected"
            );
        }
    }

    #[test]
    fn unknown_question_type_is_rejected_naming_the_valid_set() {
        let err = parse_questions(&json!({
            "questions": [{"header": "H", "question": "Q?", "type": "dropdown"}]
        }))
        .expect_err("unknown type");
        assert!(err.contains("single") && err.contains("multiple") && err.contains("text"));
    }

    #[test]
    fn choice_questions_require_between_two_and_max_options() {
        let with_options = |options: Value| {
            json!({
                "questions": [{
                    "header": "H", "question": "Q?", "type": "single", "options": options
                }]
            })
        };

        // Absent, empty and single-option all fail: none of them is a choice.
        assert!(parse_questions(&json!({
            "questions": [{"header": "H", "question": "Q?", "type": "single"}]
        }))
        .is_err());
        assert!(parse_questions(&with_options(json!([]))).is_err());
        assert!(parse_questions(&with_options(json!([{"label": "only"}]))).is_err());

        let too_many: Vec<Value> =
            std::iter::repeat_n(json!({"label": "x"}), MAX_OPTIONS + 1).collect();
        assert!(parse_questions(&with_options(json!(too_many))).is_err());

        // The boundaries themselves are accepted.
        let at_min: Vec<Value> = std::iter::repeat_n(json!({"label": "x"}), MIN_OPTIONS).collect();
        assert!(parse_questions(&with_options(json!(at_min))).is_ok());
        let at_max: Vec<Value> = std::iter::repeat_n(json!({"label": "x"}), MAX_OPTIONS).collect();
        assert!(parse_questions(&with_options(json!(at_max))).is_ok());
    }

    #[test]
    fn blank_option_label_is_rejected() {
        let err = parse_questions(&json!({
            "questions": [{
                "header": "H", "question": "Q?", "type": "single",
                "options": [{"label": "ok"}, {"label": "   "}]
            }]
        }))
        .expect_err("blank label");
        assert!(
            err.contains("option 2"),
            "the error must locate the bad option: {err:?}"
        );
    }

    #[test]
    fn text_question_carrying_options_is_rejected() {
        // The panel renders a textarea for `text`, so options would be invisible —
        // report the mistake instead of silently dropping them.
        assert!(parse_questions(&json!({
            "questions": [{
                "header": "H", "question": "Q?", "type": "text",
                "options": [{"label": "a"}, {"label": "b"}]
            }]
        }))
        .is_err());
    }

    #[test]
    fn format_answers_pairs_model_wording_with_selected_values() {
        let questions = parse_questions(&single_question_args()).unwrap();
        let text = format_answers(
            &questions,
            &[QuestionAnswer {
                question_id: "q0".to_string(),
                values: vec!["SQLite".to_string()],
            }],
        );
        assert!(text.contains("Which storage backend should the cache use?"));
        assert!(text.contains("- SQLite"));
        assert!(!text.contains("(not answered)"));
    }

    #[test]
    fn format_answers_lists_every_value_of_a_multi_select() {
        let questions = parse_questions(&json!({
            "questions": [{
                "header": "Targets",
                "question": "Which platforms?",
                "type": "multiple",
                "options": [{"label": "macOS"}, {"label": "Windows"}, {"label": "Linux"}]
            }]
        }))
        .unwrap();
        let text = format_answers(
            &questions,
            &[QuestionAnswer {
                question_id: "q0".to_string(),
                values: vec!["macOS".to_string(), "Linux".to_string()],
            }],
        );
        assert!(text.contains("- macOS") && text.contains("- Linux"));
        assert!(!text.contains("Windows"));
    }

    #[test]
    fn format_answers_marks_missing_and_blank_answers_as_unanswered() {
        // A partial submission must never read as a complete one.
        let questions = parse_questions(&json!({
            "questions": [
                {"header": "A", "question": "First?", "type": "text"},
                {"header": "B", "question": "Second?", "type": "text"}
            ]
        }))
        .unwrap();
        let text = format_answers(
            &questions,
            &[QuestionAnswer {
                question_id: "q0".to_string(),
                // Whitespace-only is not an answer.
                values: vec!["   ".to_string()],
            }],
        );
        assert_eq!(
            text.matches("(not answered)").count(),
            2,
            "a blank value and a missing entry both read as unanswered: {text:?}"
        );
    }

    /// Answers are matched by id, so a frontend that submits them out of order
    /// (or omits one) still lands each value on the right question.
    #[test]
    fn format_answers_matches_by_id_not_by_position() {
        let questions = parse_questions(&json!({
            "questions": [
                {"header": "A", "question": "First?", "type": "text"},
                {"header": "B", "question": "Second?", "type": "text"}
            ]
        }))
        .unwrap();
        let text = format_answers(
            &questions,
            &[
                QuestionAnswer {
                    question_id: "q1".to_string(),
                    values: vec!["second-answer".to_string()],
                },
                QuestionAnswer {
                    question_id: "q0".to_string(),
                    values: vec!["first-answer".to_string()],
                },
            ],
        );
        let first = text.find("First?").unwrap();
        let first_answer = text.find("first-answer").unwrap();
        let second = text.find("Second?").unwrap();
        assert!(first < first_answer && first_answer < second);
    }

    /// An unknown id is ignored rather than injected: the frontend cannot smuggle
    /// arbitrary text into the result under a question the model never asked.
    #[test]
    fn format_answers_ignores_answers_for_unknown_question_ids() {
        let questions = parse_questions(&single_question_args()).unwrap();
        let text = format_answers(
            &questions,
            &[QuestionAnswer {
                question_id: "q99".to_string(),
                values: vec!["injected".to_string()],
            }],
        );
        assert!(!text.contains("injected"));
        assert!(text.contains("(not answered)"));
    }

    #[tokio::test]
    async fn invalid_arguments_error_without_emitting_or_parking() {
        let emitted = Arc::new(Mutex::new(Vec::<Value>::new()));
        let sink = Arc::clone(&emitted);
        let emitter: QuestionEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let result = execute_ask_question("s-invalid", Some(&emitter), "call-1", json!({})).await;
        assert!(get_text(&result).contains("`questions`"));
        assert!(
            emitted.lock().unwrap().is_empty(),
            "a rejected call must not open a panel"
        );
    }

    #[tokio::test]
    async fn no_emitter_fails_open_telling_the_model_to_ask_in_prose() {
        let result = execute_ask_question("s-no-ui", None, "call-1", single_question_args()).await;
        let text = get_text(&result);
        assert_eq!(text, NO_SURFACE_MSG);
        // Fail-OPEN: the guidance must point at conversation, not refusal.
        assert!(text.contains("plain text"));
    }

    /// A fake emitter recording every request, so a test can read back the
    /// payload (and its `requestId`) without a live Tauri window.
    fn recording_emitter() -> (QuestionEmitter, Arc<Mutex<Vec<Value>>>) {
        let recorded: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);
        (
            Arc::new(move |payload| sink.lock().unwrap().push(payload)),
            recorded,
        )
    }

    /// Spin until the emitter captured a request, then return its `requestId`.
    /// Bounded so a wiring regression fails loudly instead of hanging.
    async fn await_request_id(recorded: &Arc<Mutex<Vec<Value>>>) -> String {
        for _ in 0..1000 {
            if let Some(req) = recorded.lock().unwrap().first() {
                return req
                    .get("requestId")
                    .and_then(Value::as_str)
                    .expect("request must carry a requestId")
                    .to_string();
            }
            tokio::task::yield_now().await;
        }
        panic!("no agent_question_request was emitted within the bound");
    }

    /// Drive the tool body on a background task and resolve it with `respond`,
    /// returning the result the model would see plus the emitted payload — the
    /// frontend round-trip in miniature.
    async fn round_trip(
        session_id: &str,
        respond: impl FnOnce(&str) + Send + 'static,
    ) -> (String, Value) {
        let (emitter, recorded) = recording_emitter();
        let session = session_id.to_string();
        let task = tokio::spawn(async move {
            execute_ask_question(&session, Some(&emitter), "call-1", single_question_args()).await
        });

        let request_id = await_request_id(&recorded).await;
        respond(&request_id);

        let result = task.await.expect("tool task joins");
        let payload = recorded.lock().unwrap()[0].clone();
        (get_text(&result).to_string(), payload)
    }

    #[tokio::test]
    async fn answered_round_trip_emits_the_request_and_returns_the_answer() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (text, payload) = round_trip(&session_id, |request_id| {
            respond_to_question(
                request_id,
                QuestionResponse::Answered {
                    answers: vec![QuestionAnswer {
                        question_id: "q0".to_string(),
                        values: vec!["SQLite".to_string()],
                    }],
                },
            );
        })
        .await;

        // The `agent_question_request` shape the frontend consumes.
        assert_eq!(payload.get("sessionId").unwrap(), session_id.as_str());
        assert_eq!(payload.get("callId").unwrap(), "call-1");
        let questions = payload.get("questions").unwrap().as_array().unwrap();
        assert_eq!(questions.len(), 1);
        // Serialized for the frontend: `type` (not `kind`) with snake_case values.
        assert_eq!(questions[0].get("type").unwrap(), "single");
        assert_eq!(questions[0].get("id").unwrap(), "q0");

        assert!(text.contains("- SQLite"));
    }

    #[tokio::test]
    async fn dismissed_round_trip_reads_as_a_user_choice_not_a_failure() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (text, _) = round_trip(&session_id, |request_id| {
            respond_to_question(request_id, QuestionResponse::Dismissed);
        })
        .await;

        assert!(text.contains("dismissed"));
        // Must not read as a malfunction, or the model reports a broken tool.
        for failure_word in ["error", "failed"] {
            assert!(
                !text.contains(failure_word),
                "dismissal must read as a choice, contained {failure_word:?}: {text:?}"
            );
        }
    }

    #[tokio::test]
    async fn abort_cancels_a_parked_question() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let cancel_session = session_id.clone();
        let (text, _) = round_trip(&session_id, move |_request_id| {
            cancel_pending_questions_for_session(&cancel_session);
        })
        .await;

        assert_eq!(text, CANCELLED_MSG);
    }

    /// Cancellation is session-scoped: aborting one session must not drop another
    /// session's open panel.
    #[tokio::test]
    async fn cancel_only_touches_the_named_session() {
        let keep = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<QuestionResponse>();
        let request_id = uuid::Uuid::new_v4().to_string();
        pending_questions().lock().unwrap().insert(
            request_id.clone(),
            PendingQuestion {
                session_id: keep.clone(),
                sender: tx,
            },
        );

        cancel_pending_questions_for_session("some-other-session");
        assert!(
            pending_questions()
                .lock()
                .unwrap()
                .contains_key(&request_id),
            "another session's abort must leave this request pending"
        );

        respond_to_question(&request_id, QuestionResponse::Dismissed);
        assert_eq!(rx.await, Ok(QuestionResponse::Dismissed));
    }

    #[tokio::test]
    async fn respond_is_idempotent_for_duplicate_and_unknown_ids() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<QuestionResponse>();
        pending_questions().lock().unwrap().insert(
            request_id.clone(),
            PendingQuestion {
                session_id: "idempotent-session".to_string(),
                sender: tx,
            },
        );

        respond_to_question(&request_id, QuestionResponse::Dismissed);
        assert_eq!(rx.await, Ok(QuestionResponse::Dismissed));

        // Duplicate and unknown ids are clean no-ops.
        respond_to_question(&request_id, QuestionResponse::Dismissed);
        respond_to_question("no-such-request-id", QuestionResponse::Dismissed);
        assert!(!pending_questions()
            .lock()
            .unwrap()
            .contains_key(&request_id));
    }

    /// The wire shape `agent_question_respond` deserializes; pin it so a rename
    /// is a deliberate IPC change rather than a silent break.
    #[test]
    fn question_response_wire_shape_is_stable() {
        let answered: QuestionResponse = serde_json::from_value(json!({
            "kind": "answered",
            "answers": [{"questionId": "q0", "values": ["a", "b"]}]
        }))
        .expect("answered shape deserializes");
        assert_eq!(
            answered,
            QuestionResponse::Answered {
                answers: vec![QuestionAnswer {
                    question_id: "q0".to_string(),
                    values: vec!["a".to_string(), "b".to_string()],
                }]
            }
        );

        let dismissed: QuestionResponse =
            serde_json::from_value(json!({"kind": "dismissed"})).expect("dismissed deserializes");
        assert_eq!(dismissed, QuestionResponse::Dismissed);

        // `values` may be omitted entirely (an untouched question).
        let sparse: QuestionResponse = serde_json::from_value(json!({
            "kind": "answered",
            "answers": [{"questionId": "q0"}]
        }))
        .expect("omitted values default to empty");
        assert_eq!(
            sparse,
            QuestionResponse::Answered {
                answers: vec![QuestionAnswer {
                    question_id: "q0".to_string(),
                    values: Vec::new(),
                }]
            }
        );
    }

    /// The tool resolves via the registered `execute` closure (end-to-end through
    /// `AgentTool`), proving the wiring.
    #[test]
    fn ask_question_tool_execute_closure_resolves() {
        let tool = make_ask_question_tool("s-closure".to_string(), None);
        assert_eq!(tool.name, TOOL_ASK_QUESTION);
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-aq".to_string(),
            args: single_question_args(),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let result =
            super::super::test_support::tokio_test_block((tool.execute)(ctx)).expect("execute ok");
        // No emitter → the fail-open message, proving the closure ran the body.
        assert_eq!(get_text(&result), NO_SURFACE_MSG);
    }

    /// A verbatim `ask_question` call as produced by a real model (deepseek-chat)
    /// from the shipped description + schema. Pins the round trip against actual
    /// model output rather than only against hand-written arguments: a schema or
    /// description change that models answer differently should be a deliberate
    /// edit here.
    fn real_model_args() -> Value {
        json!({
            "questions": [
                {
                    "header": "导出格式",
                    "question": "导出的文件格式希望是哪种？",
                    "type": "single",
                    "options": [
                        {"label": "CSV", "description": "通用性好，可用 Excel 直接打开，适合表格类数据"},
                        {"label": "Excel (.xlsx)", "description": "保留样式和多个工作表，文件更正式但体积稍大"},
                        {"label": "PDF", "description": "适合报表/预览类导出，不可编辑"},
                        {"label": "JSON", "description": "机器可读，适合数据备份或二次处理"}
                    ]
                },
                {
                    "header": "导出范围",
                    "question": "导出的数据范围包含哪些？（可多选）",
                    "type": "multiple",
                    "options": [
                        {"label": "当前筛选结果", "description": "只导出用户当前选中/过滤后的数据"},
                        {"label": "全量数据", "description": "导出全部数据，不理会筛选状态"},
                        {"label": "用户勾选项", "description": "只导出用户手动勾选的行"},
                        {"label": "带字段选项", "description": "导出前让用户自选包含哪些字段列"}
                    ]
                },
                {
                    "header": "导出细节",
                    "question": "关于导出功能，你还有哪些具体需求或约束需要补充说明？（自由填写）",
                    "type": "text"
                }
            ]
        })
    }

    #[test]
    fn real_model_output_parses_into_all_three_question_kinds() {
        let questions = parse_questions(&real_model_args()).expect("real model output parses");
        assert_eq!(questions.len(), 3);

        let kinds: Vec<QuestionKind> = questions.iter().map(|q| q.kind).collect();
        assert_eq!(
            kinds,
            vec![
                QuestionKind::Single,
                QuestionKind::Multiple,
                QuestionKind::Text
            ]
        );
        assert_eq!(questions[0].options.len(), 4);
        assert_eq!(
            questions[0].options[0].description.as_deref(),
            Some("通用性好，可用 Excel 直接打开，适合表格类数据")
        );
        // The model correctly omitted `options` on the text question.
        assert!(questions[2].options.is_empty());
    }

    /// The full round trip on real model output: emit → answer each kind →
    /// result text. Mirrors what the panel submits (one selection, two
    /// selections, one typed reply).
    #[tokio::test]
    async fn real_model_call_round_trips_through_the_panel_contract() {
        let (emitter, recorded) = recording_emitter();
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = session_id.clone();
        let task = tokio::spawn(async move {
            execute_ask_question(&session, Some(&emitter), "call-live", real_model_args()).await
        });

        let request_id = await_request_id(&recorded).await;
        respond_to_question(
            &request_id,
            QuestionResponse::Answered {
                answers: vec![
                    QuestionAnswer {
                        question_id: "q0".to_string(),
                        values: vec!["CSV".to_string()],
                    },
                    QuestionAnswer {
                        question_id: "q1".to_string(),
                        values: vec!["当前筛选结果".to_string(), "带字段选项".to_string()],
                    },
                    QuestionAnswer {
                        question_id: "q2".to_string(),
                        values: vec!["导出要支持后台任务，超过 1 万行时异步生成".to_string()],
                    },
                ],
            },
        );

        let result = task.await.expect("tool task joins");
        let text = get_text(&result);

        // Every question is reported, paired with the model's own wording.
        assert!(text.contains("导出的文件格式希望是哪种？"));
        assert!(text.contains("- CSV"));
        assert!(text.contains("- 当前筛选结果") && text.contains("- 带字段选项"));
        assert!(text.contains("导出要支持后台任务，超过 1 万行时异步生成"));
        assert!(!text.contains("(not answered)"));

        // The panel receives the three questions with their wire `type` tags.
        let payload = recorded.lock().unwrap()[0].clone();
        let emitted = payload.get("questions").unwrap().as_array().unwrap();
        let types: Vec<&str> = emitted
            .iter()
            .map(|q| q.get("type").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(types, vec!["single", "multiple", "text"]);
    }

    #[test]
    fn event_and_tool_names_are_the_ipc_contract() {
        // Pinned: the frontend listens on / gates by these exact strings.
        assert_eq!(TOOL_ASK_QUESTION, "ask_question");
        assert_eq!(QUESTION_REQUEST_EVENT, "agent_question_request");
    }
}
