use serde::Deserialize;
use serde_json::Value;

/// A complete Gemini CLI session file.
#[derive(Debug, Deserialize)]
pub struct GeminiSession {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "projectHash")]
    pub project_hash: String,
    #[serde(rename = "projectPath", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub messages: Vec<GeminiMessage>,
}

/// A single message within a Gemini session.
#[derive(Debug, Deserialize)]
pub struct GeminiMessage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(rename = "toolCalls", default)]
    pub tool_calls: Vec<GeminiToolCall>,
    #[serde(default)]
    pub thoughts: Vec<GeminiThought>,
    #[serde(default)]
    pub tokens: Option<GeminiTokens>,
    #[serde(default)]
    pub model: Option<String>,
}

/// A tool invocation within a Gemini assistant message.
#[derive(Debug, Deserialize)]
pub struct GeminiToolCall {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub args: Option<Value>,
    #[serde(default)]
    pub result: Vec<GeminiToolResult>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "resultDisplay", default)]
    pub result_display: Option<String>,
}

/// A result entry within a Gemini tool call.
#[derive(Debug, Deserialize)]
pub struct GeminiToolResult {
    #[serde(rename = "functionResponse")]
    pub function_response: GeminiFunctionResponse,
}

/// The function response payload within a tool result.
#[derive(Debug, Deserialize)]
pub struct GeminiFunctionResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub response: Option<Value>,
}

/// A thought entry within a Gemini assistant message.
#[derive(Debug, Deserialize)]
pub struct GeminiThought {
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// Token usage statistics for a Gemini assistant message.
#[derive(Debug, Deserialize)]
pub struct GeminiTokens {
    #[serde(default)]
    pub input: Option<u64>,
    #[serde(default)]
    pub output: Option<u64>,
    #[serde(default)]
    pub cached: Option<u64>,
    #[serde(default)]
    pub thoughts: Option<u64>,
    #[serde(default)]
    pub tool: Option<u64>,
    #[serde(default)]
    pub total: Option<u64>,
}

/// Parse a Gemini session JSON file from raw bytes.
pub fn parse_session(data: &[u8]) -> anyhow::Result<GeminiSession> {
    let session: GeminiSession = serde_json::from_slice(data)?;
    Ok(session)
}

/// Extract tool output text from a GeminiToolCall.
///
/// Uses `resultDisplay` first, then falls back to the `functionResponse.response`
/// field (extracting `output` or `error` from it).
pub fn extract_tool_output(tool_call: &GeminiToolCall) -> Option<String> {
    // Prefer resultDisplay
    if let Some(display) = &tool_call.result_display {
        if !display.is_empty() {
            return Some(display.clone());
        }
    }

    // Fall back to functionResponse.response
    for result in &tool_call.result {
        if let Some(response) = &result.function_response.response {
            // Try "output" key first, then "error"
            if let Some(output) = response.get("output") {
                return Some(value_to_string(output));
            }
            if let Some(error) = response.get("error") {
                return Some(value_to_string(error));
            }
            // If response is a scalar or other shape, stringify it
            return Some(value_to_string(response));
        }
    }

    None
}

/// Convert a serde_json Value to a human-readable string.
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_minimal_session() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "abc-123",
            "projectHash": "deadbeef",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": []
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.session_id, "abc-123");
        assert_eq!(session.project_hash, "deadbeef");
        assert!(session.project_path.is_none());
        assert!(session.cwd.is_none());
        assert!(session.summary.is_none());
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_parse_project_path_metadata() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "abc-123",
            "projectHash": "deadbeef",
            "projectPath": "D:/VSP/agentswap-gui",
            "cwd": "D:/VSP/fallback",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": []
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.project_path.as_deref(), Some("D:/VSP/agentswap-gui"));
        assert_eq!(session.cwd.as_deref(), Some("D:/VSP/fallback"));
    }

    #[test]
    fn test_parse_session_with_summary() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "abc-123",
            "projectHash": "deadbeef",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "summary": "Debugging session",
            "messages": []
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.summary.as_deref(), Some("Debugging session"));
    }

    #[test]
    fn test_parse_user_message() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "s1",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "msg-1",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "user",
                    "content": "Hello Gemini"
                }
            ]
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].message_type, "user");
        assert_eq!(session.messages[0].content.as_deref(), Some("Hello Gemini"));
    }

    #[test]
    fn test_parse_gemini_message_with_tool_calls() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "s1",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "msg-2",
                    "timestamp": "2026-03-04T10:00:02.000Z",
                    "type": "gemini",
                    "content": "",
                    "model": "gemini-2.5-pro",
                    "thoughts": [
                        {
                            "subject": "Planning",
                            "description": "I need to read the file first",
                            "timestamp": "2026-03-04T10:00:02.100Z"
                        }
                    ],
                    "tokens": {
                        "input": 100,
                        "output": 50,
                        "cached": 0,
                        "thoughts": 20,
                        "tool": 10,
                        "total": 180
                    },
                    "toolCalls": [
                        {
                            "id": "tc-1",
                            "name": "readFile",
                            "args": {"path": "/tmp/test.rs"},
                            "result": [
                                {
                                    "functionResponse": {
                                        "id": "tc-1",
                                        "name": "readFile",
                                        "response": {"output": "fn main() {}"}
                                    }
                                }
                            ],
                            "status": "success",
                            "timestamp": "2026-03-04T10:00:02.500Z",
                            "displayName": "Read File",
                            "description": "Reading test.rs",
                            "resultDisplay": "fn main() {}"
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.messages.len(), 1);
        let msg = &session.messages[0];
        assert_eq!(msg.message_type, "gemini");
        assert_eq!(msg.content.as_deref(), Some(""));
        assert_eq!(msg.model.as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(msg.thoughts.len(), 1);
        assert_eq!(msg.thoughts[0].subject.as_deref(), Some("Planning"));
        assert!(msg.tokens.is_some());
        let tokens = msg.tokens.as_ref().unwrap();
        assert_eq!(tokens.input, Some(100));
        assert_eq!(tokens.total, Some(180));
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.tool_calls[0].name, "readFile");
        assert_eq!(msg.tool_calls[0].status.as_deref(), Some("success"));
        assert_eq!(
            msg.tool_calls[0].result_display.as_deref(),
            Some("fn main() {}")
        );
    }

    #[test]
    fn test_parse_info_message() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "s1",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "msg-3",
                    "timestamp": "2026-03-04T10:00:03.000Z",
                    "type": "info",
                    "content": "MCP server connected"
                }
            ]
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.messages[0].message_type, "info");
        assert_eq!(
            session.messages[0].content.as_deref(),
            Some("MCP server connected")
        );
    }

    #[test]
    fn test_parse_error_message() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "s1",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "msg-4",
                    "timestamp": "2026-03-04T10:00:04.000Z",
                    "type": "error",
                    "content": "Something went wrong"
                }
            ]
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.messages[0].message_type, "error");
        assert_eq!(
            session.messages[0].content.as_deref(),
            Some("Something went wrong")
        );
    }

    #[test]
    fn test_extract_tool_output_result_display() {
        let tc = GeminiToolCall {
            id: Some("tc-1".to_string()),
            name: "readFile".to_string(),
            args: None,
            result: vec![],
            status: Some("success".to_string()),
            timestamp: None,
            display_name: None,
            description: None,
            result_display: Some("file contents here".to_string()),
        };
        assert_eq!(
            extract_tool_output(&tc),
            Some("file contents here".to_string())
        );
    }

    #[test]
    fn test_extract_tool_output_function_response_output() {
        let tc = GeminiToolCall {
            id: Some("tc-1".to_string()),
            name: "readFile".to_string(),
            args: None,
            result: vec![GeminiToolResult {
                function_response: GeminiFunctionResponse {
                    id: Some("tc-1".to_string()),
                    name: Some("readFile".to_string()),
                    response: Some(json!({"output": "fn main() {}"})),
                },
            }],
            status: Some("success".to_string()),
            timestamp: None,
            display_name: None,
            description: None,
            result_display: None,
        };
        assert_eq!(extract_tool_output(&tc), Some("fn main() {}".to_string()));
    }

    #[test]
    fn test_extract_tool_output_function_response_error() {
        let tc = GeminiToolCall {
            id: Some("tc-1".to_string()),
            name: "readFile".to_string(),
            args: None,
            result: vec![GeminiToolResult {
                function_response: GeminiFunctionResponse {
                    id: Some("tc-1".to_string()),
                    name: Some("readFile".to_string()),
                    response: Some(json!({"error": "file not found"})),
                },
            }],
            status: Some("error".to_string()),
            timestamp: None,
            display_name: None,
            description: None,
            result_display: None,
        };
        assert_eq!(extract_tool_output(&tc), Some("file not found".to_string()));
    }

    #[test]
    fn test_extract_tool_output_prefers_result_display() {
        let tc = GeminiToolCall {
            id: Some("tc-1".to_string()),
            name: "readFile".to_string(),
            args: None,
            result: vec![GeminiToolResult {
                function_response: GeminiFunctionResponse {
                    id: Some("tc-1".to_string()),
                    name: Some("readFile".to_string()),
                    response: Some(json!({"output": "raw output"})),
                },
            }],
            status: Some("success".to_string()),
            timestamp: None,
            display_name: None,
            description: None,
            result_display: Some("formatted display".to_string()),
        };
        assert_eq!(
            extract_tool_output(&tc),
            Some("formatted display".to_string())
        );
    }

    #[test]
    fn test_extract_tool_output_none_when_empty() {
        let tc = GeminiToolCall {
            id: Some("tc-1".to_string()),
            name: "readFile".to_string(),
            args: None,
            result: vec![],
            status: Some("success".to_string()),
            timestamp: None,
            display_name: None,
            description: None,
            result_display: None,
        };
        assert_eq!(extract_tool_output(&tc), None);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_session(b"this is not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_gemini_message_without_optional_fields() {
        let data = serde_json::to_vec(&json!({
            "sessionId": "s1",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "type": "gemini",
                    "content": "Hello!"
                }
            ]
        }))
        .unwrap();

        let session = parse_session(&data).unwrap();
        assert_eq!(session.messages.len(), 1);
        let msg = &session.messages[0];
        assert!(msg.id.is_none());
        assert!(msg.timestamp.is_none());
        assert!(msg.model.is_none());
        assert!(msg.tokens.is_none());
        assert!(msg.tool_calls.is_empty());
        assert!(msg.thoughts.is_empty());
    }
}
