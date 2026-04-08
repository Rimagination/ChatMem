use serde::Deserialize;
use serde_json::Value;

/// A single event line from a Codex CLI JSONL rollout file.
///
/// Each line in the rollout file has the shape:
/// `{"timestamp": "...", "type": "...", "payload": {...}}`
#[derive(Debug, Deserialize)]
pub struct CodexEvent {
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Option<Value>,
}

/// Thread metadata as stored in the SQLite `threads` table.
///
/// This is not deserialized from JSON; it is populated by querying the database.
#[derive(Debug, Clone)]
pub struct CodexThread {
    pub id: String,
    pub rollout_path: String,
    pub cwd: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub tokens_used: i64,
    pub git_branch: Option<String>,
    pub first_user_message: String,
}

/// Parse a single JSONL line into a CodexEvent, returning None for lines that
/// fail to parse (malformed JSON or unexpected schema).
pub fn parse_event(line: &str) -> Option<CodexEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

/// Extract the payload subtype from a CodexEvent.
///
/// Returns the `payload.type` string if present, or `None` otherwise.
pub fn payload_type(event: &CodexEvent) -> Option<&str> {
    event
        .payload
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|v| v.as_str())
}

/// Extract a string field from the event payload.
pub fn payload_str<'a>(event: &'a CodexEvent, key: &str) -> Option<&'a str> {
    event
        .payload
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
}

/// Extract file changes from a `custom_tool_call_output` payload.
///
/// The output JSON typically looks like:
/// `{"output": "Success. Updated the following files:\nM /path/to/file\n", ...}`
///
/// Returns a list of `(change_indicator, path)` tuples, e.g. `("M", "/path/to/file")`.
pub fn extract_file_changes(output_str: &str) -> Vec<(String, String)> {
    // Try to parse as JSON first to get the "output" field
    let text = if let Ok(parsed) = serde_json::from_str::<Value>(output_str) {
        parsed
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or(output_str)
            .to_string()
    } else {
        output_str.to_string()
    };

    let mut changes = Vec::new();

    // Look for lines after "Updated the following files:"
    let mut in_file_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Updated the following files:") {
            in_file_section = true;
            continue;
        }
        if in_file_section && !trimmed.is_empty() {
            // Lines look like "M /path/to/file" or "A /path/to/file"
            if let Some((indicator, path)) = trimmed.split_once(' ') {
                changes.push((indicator.to_string(), path.to_string()));
            }
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_event_msg_user_message() {
        let line = r#"{"timestamp":"2026-03-01T19:01:30.227Z","type":"event_msg","payload":{"type":"user_message","message":"analyze the codebase","images":[],"local_images":[],"text_elements":[]}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "event_msg");
        assert_eq!(payload_type(&event), Some("user_message"));
        assert_eq!(payload_str(&event, "message"), Some("analyze the codebase"));
    }

    #[test]
    fn test_parse_event_msg_agent_message() {
        let line = r#"{"timestamp":"2026-03-01T19:01:35.949Z","type":"event_msg","payload":{"type":"agent_message","message":"I will inspect the repository.","phase":"commentary"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "event_msg");
        assert_eq!(payload_type(&event), Some("agent_message"));
        assert_eq!(
            payload_str(&event, "message"),
            Some("I will inspect the repository.")
        );
    }

    #[test]
    fn test_parse_event_msg_agent_reasoning() {
        let line = r#"{"timestamp":"2026-03-01T19:01:35.081Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"**Planning codebase inspection**"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "event_msg");
        assert_eq!(payload_type(&event), Some("agent_reasoning"));
        assert_eq!(
            payload_str(&event, "text"),
            Some("**Planning codebase inspection**")
        );
    }

    #[test]
    fn test_parse_response_item_function_call() {
        let line = r#"{"timestamp":"2026-03-01T19:01:35.959Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd && ls -la\"}","call_id":"call_abc123"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "response_item");
        assert_eq!(payload_type(&event), Some("function_call"));
        assert_eq!(payload_str(&event, "name"), Some("exec_command"));
        assert_eq!(payload_str(&event, "call_id"), Some("call_abc123"));

        // arguments is a JSON-encoded string
        let args_str = payload_str(&event, "arguments").unwrap();
        let args: Value = serde_json::from_str(args_str).unwrap();
        assert_eq!(args["cmd"], "pwd && ls -la");
    }

    #[test]
    fn test_parse_response_item_function_call_output() {
        let line = r#"{"timestamp":"2026-03-01T19:01:36.114Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_abc123","output":"Process exited with code 0\nOutput:\n/Users/test\n"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "response_item");
        assert_eq!(payload_type(&event), Some("function_call_output"));
        assert_eq!(payload_str(&event, "call_id"), Some("call_abc123"));
    }

    #[test]
    fn test_parse_response_item_custom_tool_call() {
        let line = r#"{"timestamp":"2026-03-02T20:26:12.564Z","type":"response_item","payload":{"type":"custom_tool_call","status":"completed","call_id":"call_xyz","name":"apply_patch","input":"*** Begin Patch\n*** Update File: /tmp/test.rs\n"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "response_item");
        assert_eq!(payload_type(&event), Some("custom_tool_call"));
        assert_eq!(payload_str(&event, "name"), Some("apply_patch"));
        assert_eq!(payload_str(&event, "call_id"), Some("call_xyz"));
        // input is plain text, not JSON
        assert!(payload_str(&event, "input")
            .unwrap()
            .contains("Begin Patch"));
    }

    #[test]
    fn test_parse_response_item_custom_tool_call_output() {
        let line = r#"{"timestamp":"2026-03-02T20:26:12.613Z","type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"call_xyz","output":"{\"output\":\"Success. Updated the following files:\\nM /tmp/test.rs\\n\",\"metadata\":{\"exit_code\":0}}"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "response_item");
        assert_eq!(payload_type(&event), Some("custom_tool_call_output"));
        assert_eq!(payload_str(&event, "call_id"), Some("call_xyz"));
    }

    #[test]
    fn test_parse_session_meta() {
        let line = r#"{"timestamp":"2026-03-01T19:01:30.226Z","type":"session_meta","payload":{"id":"019caac6-dee9","cwd":"/Users/test"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "session_meta");
    }

    #[test]
    fn test_parse_turn_context() {
        let line = r#"{"timestamp":"2026-03-01T19:01:30.230Z","type":"turn_context","payload":{"turn_id":"019caac6-ef16","cwd":"/Users/test","model":"gpt-5.3-codex"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "turn_context");
    }

    #[test]
    fn test_parse_event_msg_token_count() {
        let line = r#"{"timestamp":"2026-03-01T19:01:31.414Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "event_msg");
        assert_eq!(payload_type(&event), Some("token_count"));
    }

    #[test]
    fn test_parse_event_msg_task_started() {
        let line = r#"{"timestamp":"2026-03-01T19:01:30.227Z","type":"event_msg","payload":{"type":"task_started","turn_id":"019caac6-ef16"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(payload_type(&event), Some("task_started"));
    }

    #[test]
    fn test_parse_event_msg_task_complete() {
        let line = r#"{"timestamp":"2026-03-01T19:02:48.213Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"019caac6-ef16","last_agent_message":"Done."}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(payload_type(&event), Some("task_complete"));
    }

    #[test]
    fn test_parse_response_item_reasoning() {
        let line = r#"{"timestamp":"2026-03-01T19:01:35.081Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"Planning"}],"content":null}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(payload_type(&event), Some("reasoning"));
    }

    #[test]
    fn test_parse_response_item_message_skip() {
        let line = r#"{"timestamp":"2026-03-01T19:01:30.227Z","type":"response_item","payload":{"type":"message","role":"developer","content":[]}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "response_item");
        assert_eq!(payload_type(&event), Some("message"));
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(parse_event("").is_none());
        assert!(parse_event("   ").is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        assert!(parse_event("not json at all").is_none());
        assert!(parse_event("{incomplete").is_none());
    }

    #[test]
    fn test_extract_file_changes_from_json_output() {
        let output = r#"{"output":"Success. Updated the following files:\nM /Users/test/src/main.rs\n","metadata":{"exit_code":0}}"#;
        let changes = extract_file_changes(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "M");
        assert_eq!(changes[0].1, "/Users/test/src/main.rs");
    }

    #[test]
    fn test_extract_file_changes_multiple_files() {
        let output = json!({
            "output": "Success. Updated the following files:\nM /tmp/a.rs\nA /tmp/b.rs\nM /tmp/c.rs\n"
        })
        .to_string();
        let changes = extract_file_changes(&output);
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0], ("M".to_string(), "/tmp/a.rs".to_string()));
        assert_eq!(changes[1], ("A".to_string(), "/tmp/b.rs".to_string()));
        assert_eq!(changes[2], ("M".to_string(), "/tmp/c.rs".to_string()));
    }

    #[test]
    fn test_extract_file_changes_no_file_section() {
        let output = r#"{"output":"Command completed successfully.\n"}"#;
        let changes = extract_file_changes(output);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_extract_file_changes_plain_text() {
        let output = "Success. Updated the following files:\nM /tmp/test.rs\n";
        let changes = extract_file_changes(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, "/tmp/test.rs");
    }

    #[test]
    fn test_payload_type_none_for_missing_payload() {
        let event = CodexEvent {
            timestamp: None,
            event_type: "session_meta".to_string(),
            payload: None,
        };
        assert_eq!(payload_type(&event), None);
    }

    #[test]
    fn test_payload_type_none_for_no_type_field() {
        let event = CodexEvent {
            timestamp: None,
            event_type: "session_meta".to_string(),
            payload: Some(json!({"id": "abc"})),
        };
        assert_eq!(payload_type(&event), None);
    }

    #[test]
    fn test_codex_thread_fields() {
        let thread = CodexThread {
            id: "019caac6-dee9".to_string(),
            rollout_path: "/home/user/.codex/sessions/2026/03/02/rollout.jsonl".to_string(),
            cwd: "/home/user/project".to_string(),
            title: "Analyze codebase".to_string(),
            created_at: 1770798002,
            updated_at: 1770798114,
            tokens_used: 53708,
            git_branch: Some("main".to_string()),
            first_user_message: "analyze the current codebase".to_string(),
        };
        assert_eq!(thread.id, "019caac6-dee9");
        assert_eq!(thread.tokens_used, 53708);
        assert_eq!(thread.git_branch.as_deref(), Some("main"));
    }
}
