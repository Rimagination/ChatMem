use serde::Deserialize;
use serde_json::Value;

/// A single event line from a Claude Code JSONL session file.
#[derive(Debug, Deserialize)]
pub struct ClaudeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub uuid: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub message: Option<ClaudeMessage>,
    pub summary: Option<String>,
    #[serde(rename = "leafUuid")]
    pub leaf_uuid: Option<String>,
    pub snapshot: Option<Value>,
    #[serde(rename = "isSidechain", default)]
    pub is_sidechain: bool,
}

/// The message payload within a Claude event.
#[derive(Debug, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub id: Option<String>,
    pub content: ClaudeContent,
}

/// Content of a Claude message: either a plain string (user text) or an array
/// of typed content blocks (assistant blocks or tool results).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ClaudeContent {
    Text(String),
    Blocks(Vec<ClaudeContentBlock>),
}

/// A single content block within a Claude message.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<ToolResultContent>,
    },
}

/// The content of a tool result: either a plain string or an arbitrary JSON value.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Other(Value),
}

/// Parse a single JSONL line into a ClaudeEvent, returning None for lines that
/// fail to parse (malformed JSON or unexpected schema).
pub fn parse_event(line: &str) -> Option<ClaudeEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

/// Decode a Claude path-encoded project directory name back to the original path.
///
/// Claude encodes `/Users/foo/bar` as `-Users-foo-bar`, and Windows paths
/// such as `D:\VSP` as `D--VSP`.
pub fn decode_project_path(encoded: &str) -> String {
    if encoded.is_empty() {
        return String::new();
    }
    if encoded.len() >= 3 {
        let bytes = encoded.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b'-' && bytes[2] == b'-' {
            return format!(
                "{}:/{}",
                encoded.chars().next().unwrap(),
                encoded[3..].replace('-', "/")
            );
        }
    }
    // Replace leading dash and all dashes with path separators.
    // The encoded string starts with `-` which becomes `/`.
    encoded.replace('-', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_path() {
        assert_eq!(
            decode_project_path("-Users-nimishgj-github"),
            "/Users/nimishgj/github"
        );
        assert_eq!(decode_project_path("D--VSP"), "D:/VSP");
        assert_eq!(
            decode_project_path("C--Users-Liang-Documents-Code"),
            "C:/Users/Liang/Documents/Code"
        );
        assert_eq!(decode_project_path("-private-tmp"), "/private/tmp");
        assert_eq!(decode_project_path(""), "");
    }

    #[test]
    fn test_parse_user_text_message() {
        let line = r#"{"type":"user","uuid":"abc","timestamp":"2026-03-04T15:01:30.141Z","isSidechain":false,"message":{"role":"user","content":"Hello world"}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "user");
        assert_eq!(event.uuid.as_deref(), Some("abc"));
        assert!(!event.is_sidechain);
        let msg = event.message.unwrap();
        assert_eq!(msg.role, "user");
        match msg.content {
            ClaudeContent::Text(t) => assert_eq!(t, "Hello world"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_parse_assistant_text_block() {
        let line = r#"{"type":"assistant","uuid":"def","timestamp":"2026-03-04T15:01:37.423Z","isSidechain":false,"message":{"role":"assistant","id":"msg_123","content":[{"type":"text","text":"Here is my response."}]}}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "assistant");
        let msg = event.message.unwrap();
        assert_eq!(msg.id.as_deref(), Some("msg_123"));
        match msg.content {
            ClaudeContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ClaudeContentBlock::Text { text } => assert_eq!(text, "Here is my response."),
                    _ => panic!("Expected Text block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn test_parse_tool_use_block() {
        let line = r#"{"type":"assistant","uuid":"ghi","timestamp":"2026-03-04T15:01:38.339Z","isSidechain":false,"message":{"role":"assistant","id":"msg_456","content":[{"type":"tool_use","id":"toolu_abc","name":"Read","input":{"file_path":"/tmp/test.rs"}}]}}"#;
        let event = parse_event(line).unwrap();
        let msg = event.message.unwrap();
        match msg.content {
            ClaudeContent::Blocks(blocks) => match &blocks[0] {
                ClaudeContentBlock::ToolUse { id, name, input } => {
                    assert_eq!(id, "toolu_abc");
                    assert_eq!(name, "Read");
                    assert_eq!(input["file_path"], "/tmp/test.rs");
                }
                _ => panic!("Expected ToolUse block"),
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn test_parse_tool_result_block() {
        let line = r#"{"type":"user","uuid":"jkl","timestamp":"2026-03-04T15:01:38.346Z","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_abc","content":"File contents here"}]}}"#;
        let event = parse_event(line).unwrap();
        let msg = event.message.unwrap();
        match msg.content {
            ClaudeContent::Blocks(blocks) => match &blocks[0] {
                ClaudeContentBlock::ToolResult {
                    tool_use_id,
                    content,
                } => {
                    assert_eq!(tool_use_id, "toolu_abc");
                    match content.as_ref().unwrap() {
                        ToolResultContent::Text(t) => assert_eq!(t, "File contents here"),
                        _ => panic!("Expected Text result content"),
                    }
                }
                _ => panic!("Expected ToolResult block"),
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn test_parse_summary_event() {
        let line = r#"{"type":"summary","summary":"Checkout Service 5xx Error Investigation","leafUuid":"1b3c830a-a1c3-4f42-a8df-3264e410419a"}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "summary");
        assert_eq!(
            event.summary.as_deref(),
            Some("Checkout Service 5xx Error Investigation")
        );
        assert_eq!(
            event.leaf_uuid.as_deref(),
            Some("1b3c830a-a1c3-4f42-a8df-3264e410419a")
        );
    }

    #[test]
    fn test_parse_thinking_block() {
        let line = r#"{"type":"assistant","uuid":"mno","timestamp":"2026-03-04T15:01:37.423Z","isSidechain":false,"message":{"role":"assistant","id":"msg_789","content":[{"type":"thinking","thinking":"Let me think about this...","signature":"abc123"}]}}"#;
        let event = parse_event(line).unwrap();
        let msg = event.message.unwrap();
        match msg.content {
            ClaudeContent::Blocks(blocks) => match &blocks[0] {
                ClaudeContentBlock::Thinking { thinking } => {
                    assert_eq!(thinking, "Let me think about this...");
                }
                _ => panic!("Expected Thinking block"),
            },
            _ => panic!("Expected Blocks content"),
        }
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
    fn test_parse_sidechain_event() {
        let line = r#"{"type":"user","uuid":"xyz","timestamp":"2026-03-04T15:01:30.141Z","isSidechain":true,"message":{"role":"user","content":"Sidechain message"}}"#;
        let event = parse_event(line).unwrap();
        assert!(event.is_sidechain);
    }

    #[test]
    fn test_parse_file_history_snapshot() {
        let line = r#"{"type":"file-history-snapshot","messageId":"abc","snapshot":{"messageId":"def","trackedFileBackups":{"src/main.rs":{"backupFileName":null,"version":1,"backupTime":"2026-03-04T15:25:28.294Z"}},"timestamp":"2026-03-04T15:08:48.403Z"},"isSnapshotUpdate":true}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.event_type, "file-history-snapshot");
        assert!(event.snapshot.is_some());
    }
}
