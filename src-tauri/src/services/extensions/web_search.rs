//! web_search — HandBox's web-search extension tool.
//!
//! Constructed HandBox-side (like the MCP tools) and injected via
//! `build_agent_session`'s `extra_tools`, not a coding-agent built-in. agent_run
//! registers it only when a search-provider API key is configured, so with no
//! key the model never sees the tool instead of burning a turn on a runtime
//! error. Unknown `provider` tags fall back to Tavily rather than erroring, so a
//! config written by a newer HandBox degrades gracefully.

use hand_agent::{AgentTool, ToolResult};
use serde_json::{json, Value};
use std::time::Duration;

/// Registration name the session's `enabled_tools` opts in with.
pub const WEB_SEARCH_TOOL_NAME: &str = "web_search";

const TAVILY_ENDPOINT: &str = "https://api.tavily.com/search";
const DEFAULT_MAX_RESULTS: u8 = 5;
const MAX_MAX_RESULTS: u8 = 10;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Guards the model's context window against unexpectedly long provider content.
const MAX_RESULT_CHARS: usize = 20_000;

/// The closure owns `provider`/`api_key` for the session's life, so a settings
/// change applies only from the next run (same as MCP bindings).
pub fn create_web_search_tool(provider: String, api_key: String) -> AgentTool {
    AgentTool::simple(
        WEB_SEARCH_TOOL_NAME,
        "Search the web for current information. Returns relevant results with \
         title, URL and a content snippet, plus a short synthesized answer when \
         available. Use this for recent events, facts you are unsure about, or \
         anything that needs up-to-date information.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_MAX_RESULTS,
                    "description": "Number of results to return (default 5)"
                }
            },
            "required": ["query"]
        }),
        "Web Search",
        move |_call_id, args| {
            let provider = provider.clone();
            let api_key = api_key.clone();
            async move { run_search(&provider, &api_key, &args).await }
        },
    )
}

/// Argument validation errors and provider failures both surface as
/// `ToolResult::error` so the model can correct itself.
async fn run_search(provider: &str, api_key: &str, args: &Value) -> ToolResult {
    let Some(query) = args.get("query").and_then(Value::as_str) else {
        return ToolResult::error("web_search requires a `query` string argument");
    };
    let query = query.trim();
    if query.is_empty() {
        return ToolResult::error("web_search `query` must not be empty");
    }
    let max_results = clamp_max_results(args.get("max_results").and_then(Value::as_u64));

    if !provider.is_empty() && provider != "tavily" {
        tracing::warn!("[web_search] unknown provider '{provider}', falling back to tavily");
    }
    match tavily_search(api_key, query, max_results).await {
        Ok(text) => ToolResult::text(text),
        Err(message) => ToolResult::error(message),
    }
}

fn clamp_max_results(raw: Option<u64>) -> u8 {
    match raw {
        Some(n) => (n.min(MAX_MAX_RESULTS as u64)).max(1) as u8,
        None => DEFAULT_MAX_RESULTS,
    }
}

/// Errors are user-actionable messages that never leak the key or response body.
async fn tavily_search(api_key: &str, query: &str, max_results: u8) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(TAVILY_ENDPOINT)
        .bearer_auth(api_key)
        .json(&build_tavily_request(query, max_results))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "web search timed out, try again".to_string()
            } else {
                format!("web search request failed: {e}")
            }
        })?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(tavily_error_message(status));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("web search returned an unreadable response: {e}"))?;
    Ok(format_tavily_response(&body))
}

/// Messages the model (and the user reading the transcript) can act on;
/// deliberately excludes the response body.
fn tavily_error_message(status: u16) -> String {
    match status {
        400 => "web search rejected the query as invalid".to_string(),
        401 => {
            "web search API key is missing or invalid — check Settings → Agent Tools".to_string()
        }
        429 => "web search rate limit exceeded, try again later".to_string(),
        432 | 433 => "web search plan limit exceeded — check your Tavily plan".to_string(),
        s => format!("web search failed with status {s}"),
    }
}

fn build_tavily_request(query: &str, max_results: u8) -> Value {
    json!({
        "query": query,
        "search_depth": "basic",
        "max_results": max_results,
        "include_answer": true,
    })
}

/// Renders numbered, model-friendly plain text, tolerating missing fields
/// (absent answer, empty results, partial items).
fn format_tavily_response(body: &Value) -> String {
    let mut out = String::new();

    if let Some(answer) = body.get("answer").and_then(Value::as_str) {
        let answer = answer.trim();
        if !answer.is_empty() {
            out.push_str("Answer: ");
            out.push_str(answer);
            out.push_str("\n\n");
        }
    }

    let results = body
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if results.is_empty() {
        out.push_str("No results found.");
        return out;
    }

    for (index, result) in results.iter().enumerate() {
        let title = result
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("(untitled)");
        let url = result.get("url").and_then(Value::as_str).unwrap_or("");
        let content = result
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        out.push_str(&format!("{}. {title}\n   {url}\n", index + 1));
        if !content.is_empty() {
            out.push_str("   ");
            out.push_str(content);
            out.push('\n');
        }
        out.push('\n');
        if out.len() > MAX_RESULT_CHARS {
            out.push_str("[additional results truncated]");
            break;
        }
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_shape_matches_registration_contract() {
        let tool = create_web_search_tool("tavily".to_string(), "tvly-key".to_string());
        assert_eq!(tool.name, WEB_SEARCH_TOOL_NAME);
        assert_eq!(tool.label, "Web Search");
        assert_eq!(tool.parameters["required"], json!(["query"]));
        assert!(tool.parameters["properties"]["query"].is_object());
    }

    #[test]
    fn tavily_request_carries_query_and_options() {
        let body = build_tavily_request("rust async", 3);
        assert_eq!(body["query"], "rust async");
        assert_eq!(body["max_results"], 3);
        assert_eq!(body["search_depth"], "basic");
        assert_eq!(body["include_answer"], true);
    }

    #[test]
    fn max_results_is_clamped_into_range() {
        assert_eq!(clamp_max_results(None), DEFAULT_MAX_RESULTS);
        assert_eq!(clamp_max_results(Some(3)), 3);
        assert_eq!(clamp_max_results(Some(100)), MAX_MAX_RESULTS);
        assert_eq!(clamp_max_results(Some(0)), 1);
    }

    #[test]
    fn format_renders_answer_and_numbered_results() {
        let body = json!({
            "answer": "Rust is a systems language.",
            "results": [
                { "title": "Rust", "url": "https://rust-lang.org", "content": "A language." },
                { "title": "Book", "url": "https://doc.rust-lang.org/book", "content": "Learn Rust." },
            ]
        });
        let text = format_tavily_response(&body);
        assert!(text.starts_with("Answer: Rust is a systems language."));
        assert!(text.contains("1. Rust\n   https://rust-lang.org\n   A language."));
        assert!(text.contains("2. Book\n   https://doc.rust-lang.org/book\n   Learn Rust."));
    }

    #[test]
    fn format_tolerates_missing_fields() {
        assert_eq!(format_tavily_response(&json!({})), "No results found.");
        assert_eq!(
            format_tavily_response(&json!({ "results": [] })),
            "No results found."
        );

        let partial = format_tavily_response(&json!({ "results": [ {} ] }));
        assert!(partial.contains("1. (untitled)"));
    }

    #[test]
    fn format_truncates_overlong_output() {
        let big = "x".repeat(9_000);
        let results: Vec<Value> = (0..10)
            .map(|i| json!({ "title": format!("r{i}"), "url": "https://e.com", "content": big }))
            .collect();
        let text = format_tavily_response(&json!({ "results": results }));
        assert!(text.ends_with("[additional results truncated]"));
        assert!(text.len() < 10 * 9_000);
    }

    #[test]
    fn error_messages_cover_common_statuses() {
        assert!(tavily_error_message(401).contains("API key"));
        assert!(tavily_error_message(429).contains("rate limit"));
        assert!(tavily_error_message(432).contains("plan limit"));
        assert!(tavily_error_message(433).contains("plan limit"));
        assert!(tavily_error_message(500).contains("500"));
    }
}
