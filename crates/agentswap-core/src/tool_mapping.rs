use serde_json::{Map, Value};

use crate::types::AgentKind;

/// A mapped tool call: remapped name and remapped input fields.
pub struct MappedTool {
    pub name: String,
    pub input: Value,
}

/// Map a tool call's name and input fields from one agent to another.
/// Returns `None` if no mapping is needed (tool name passes through as-is).
pub fn map_tool(source: &AgentKind, target: &AgentKind, name: &str, input: &Value) -> MappedTool {
    if source == target {
        return MappedTool {
            name: name.to_string(),
            input: input.clone(),
        };
    }

    // Normalize to a canonical name, then convert to target.
    let canonical = to_canonical(source, name);
    let target_name = from_canonical(target, &canonical);
    let target_input = remap_input(&canonical, source, target, input);

    MappedTool {
        name: target_name,
        input: target_input,
    }
}

/// Canonical tool names used as the intermediate representation.
enum Canonical {
    Shell,
    ReadFile,
    WriteFile,
    EditFile,
    Grep,
    Glob,
    ListDir,
    WebSearch,
    WebFetch,
    Todos,
    Unknown(String),
}

fn to_canonical(agent: &AgentKind, name: &str) -> Canonical {
    match agent {
        AgentKind::Claude => match name {
            "Bash" => Canonical::Shell,
            "Read" => Canonical::ReadFile,
            "Write" => Canonical::WriteFile,
            "Edit" => Canonical::EditFile,
            "Grep" => Canonical::Grep,
            "Glob" => Canonical::Glob,
            "WebSearch" => Canonical::WebSearch,
            "WebFetch" => Canonical::WebFetch,
            "TodoWrite" => Canonical::Todos,
            _ => Canonical::Unknown(name.to_string()),
        },
        AgentKind::Gemini => match name {
            "run_shell_command" => Canonical::Shell,
            "read_file" => Canonical::ReadFile,
            "write_file" => Canonical::WriteFile,
            "replace" => Canonical::EditFile,
            "grep_search" => Canonical::Grep,
            "glob" => Canonical::Glob,
            "list_directory" => Canonical::ListDir,
            "google_web_search" => Canonical::WebSearch,
            "web_fetch" => Canonical::WebFetch,
            "write_todos" => Canonical::Todos,
            _ => Canonical::Unknown(name.to_string()),
        },
        AgentKind::Codex => match name {
            "exec_command" => Canonical::Shell,
            "apply_patch" => Canonical::EditFile,
            _ => Canonical::Unknown(name.to_string()),
        },
    }
}

fn from_canonical(agent: &AgentKind, canonical: &Canonical) -> String {
    match agent {
        AgentKind::Claude => match canonical {
            Canonical::Shell => "Bash".to_string(),
            Canonical::ReadFile => "Read".to_string(),
            Canonical::WriteFile => "Write".to_string(),
            Canonical::EditFile => "Edit".to_string(),
            Canonical::Grep => "Grep".to_string(),
            Canonical::Glob => "Glob".to_string(),
            Canonical::ListDir => "Glob".to_string(),
            Canonical::WebSearch => "WebSearch".to_string(),
            Canonical::WebFetch => "WebFetch".to_string(),
            Canonical::Todos => "TodoWrite".to_string(),
            Canonical::Unknown(n) => n.clone(),
        },
        AgentKind::Gemini => match canonical {
            Canonical::Shell => "run_shell_command".to_string(),
            Canonical::ReadFile => "read_file".to_string(),
            Canonical::WriteFile => "write_file".to_string(),
            Canonical::EditFile => "replace".to_string(),
            Canonical::Grep => "grep_search".to_string(),
            Canonical::Glob => "glob".to_string(),
            Canonical::ListDir => "list_directory".to_string(),
            Canonical::WebSearch => "google_web_search".to_string(),
            Canonical::WebFetch => "web_fetch".to_string(),
            Canonical::Todos => "write_todos".to_string(),
            Canonical::Unknown(n) => n.clone(),
        },
        AgentKind::Codex => match canonical {
            Canonical::Shell => "exec_command".to_string(),
            Canonical::EditFile => "apply_patch".to_string(),
            Canonical::Unknown(n) => n.clone(),
            // Codex doesn't have these tools — pass through the original name
            _ => from_canonical(&AgentKind::Claude, canonical),
        },
    }
}

/// Remap input field names between agents for known tools.
fn remap_input(
    canonical: &Canonical,
    source: &AgentKind,
    target: &AgentKind,
    input: &Value,
) -> Value {
    let obj = match input.as_object() {
        Some(o) => o,
        None => return input.clone(),
    };

    let mut result = obj.clone();

    match canonical {
        Canonical::Shell => {
            // Claude: {command}, Gemini: {command}, Codex: {cmd}
            rename_field(
                &mut result,
                source,
                target,
                &[
                    (AgentKind::Claude, "command"),
                    (AgentKind::Gemini, "command"),
                    (AgentKind::Codex, "cmd"),
                ],
            );
        }
        Canonical::ReadFile | Canonical::WriteFile | Canonical::EditFile => {
            // Claude: {file_path}, Gemini: {file_path}
            // Both use file_path, so no rename needed for Claude<->Gemini.
            // But Glob uses different field names.
        }
        Canonical::Grep => {
            // Claude: {path, pattern}, Gemini: {file_path, pattern}
            rename_field(
                &mut result,
                source,
                target,
                &[
                    (AgentKind::Claude, "path"),
                    (AgentKind::Gemini, "file_path"),
                    (AgentKind::Codex, "path"),
                ],
            );
        }
        Canonical::Glob => {
            // Claude: {path, pattern}, Gemini: {dir_path, pattern}
            rename_field(
                &mut result,
                source,
                target,
                &[
                    (AgentKind::Claude, "path"),
                    (AgentKind::Gemini, "dir_path"),
                    (AgentKind::Codex, "path"),
                ],
            );
        }
        Canonical::ListDir => {
            // Gemini: {dir_path}, Claude doesn't have this (maps to Glob)
            rename_field(
                &mut result,
                source,
                target,
                &[
                    (AgentKind::Claude, "path"),
                    (AgentKind::Gemini, "dir_path"),
                    (AgentKind::Codex, "path"),
                ],
            );
        }
        _ => {}
    }

    Value::Object(result)
}

/// Rename a field from the source agent's name to the target agent's name.
fn rename_field(
    obj: &mut Map<String, Value>,
    source: &AgentKind,
    target: &AgentKind,
    mappings: &[(AgentKind, &str)],
) {
    let source_key = mappings.iter().find(|(a, _)| a == source).map(|(_, k)| *k);
    let target_key = mappings.iter().find(|(a, _)| a == target).map(|(_, k)| *k);

    if let (Some(src), Some(tgt)) = (source_key, target_key) {
        if src != tgt {
            if let Some(val) = obj.remove(src) {
                obj.insert(tgt.to_string(), val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_claude_bash_to_gemini_shell() {
        let input = json!({"command": "ls -la"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "Bash", &input);
        assert_eq!(mapped.name, "run_shell_command");
        assert_eq!(mapped.input, json!({"command": "ls -la"}));
    }

    #[test]
    fn test_claude_bash_to_codex() {
        let input = json!({"command": "npm test"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Codex, "Bash", &input);
        assert_eq!(mapped.name, "exec_command");
        assert_eq!(mapped.input, json!({"cmd": "npm test"}));
    }

    #[test]
    fn test_gemini_shell_to_claude() {
        let input = json!({"command": "cargo build"});
        let mapped = map_tool(
            &AgentKind::Gemini,
            &AgentKind::Claude,
            "run_shell_command",
            &input,
        );
        assert_eq!(mapped.name, "Bash");
        assert_eq!(mapped.input, json!({"command": "cargo build"}));
    }

    #[test]
    fn test_claude_read_to_gemini() {
        let input = json!({"file_path": "/tmp/test.rs"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "Read", &input);
        assert_eq!(mapped.name, "read_file");
        assert_eq!(mapped.input, json!({"file_path": "/tmp/test.rs"}));
    }

    #[test]
    fn test_claude_edit_to_gemini() {
        let input = json!({"file_path": "main.rs", "old_string": "foo", "new_string": "bar"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "Edit", &input);
        assert_eq!(mapped.name, "replace");
        assert_eq!(mapped.input["file_path"], "main.rs");
        assert_eq!(mapped.input["old_string"], "foo");
        assert_eq!(mapped.input["new_string"], "bar");
    }

    #[test]
    fn test_claude_grep_to_gemini() {
        let input = json!({"pattern": "TODO", "path": "/src"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "Grep", &input);
        assert_eq!(mapped.name, "grep_search");
        assert_eq!(
            mapped.input,
            json!({"pattern": "TODO", "file_path": "/src"})
        );
    }

    #[test]
    fn test_gemini_grep_to_claude() {
        let input = json!({"pattern": "TODO", "file_path": "/src"});
        let mapped = map_tool(
            &AgentKind::Gemini,
            &AgentKind::Claude,
            "grep_search",
            &input,
        );
        assert_eq!(mapped.name, "Grep");
        assert_eq!(mapped.input, json!({"pattern": "TODO", "path": "/src"}));
    }

    #[test]
    fn test_claude_glob_to_gemini() {
        let input = json!({"pattern": "*.rs", "path": "/src"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "Glob", &input);
        assert_eq!(mapped.name, "glob");
        assert_eq!(mapped.input, json!({"pattern": "*.rs", "dir_path": "/src"}));
    }

    #[test]
    fn test_claude_websearch_to_gemini() {
        let input = json!({"query": "rust async"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "WebSearch", &input);
        assert_eq!(mapped.name, "google_web_search");
        assert_eq!(mapped.input, json!({"query": "rust async"}));
    }

    #[test]
    fn test_unknown_tool_passes_through() {
        let input = json!({"foo": "bar"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Gemini, "CustomTool", &input);
        assert_eq!(mapped.name, "CustomTool");
        assert_eq!(mapped.input, json!({"foo": "bar"}));
    }

    #[test]
    fn test_same_agent_no_change() {
        let input = json!({"command": "ls"});
        let mapped = map_tool(&AgentKind::Claude, &AgentKind::Claude, "Bash", &input);
        assert_eq!(mapped.name, "Bash");
        assert_eq!(mapped.input, json!({"command": "ls"}));
    }

    #[test]
    fn test_codex_to_gemini() {
        let input = json!({"cmd": "make build"});
        let mapped = map_tool(
            &AgentKind::Codex,
            &AgentKind::Gemini,
            "exec_command",
            &input,
        );
        assert_eq!(mapped.name, "run_shell_command");
        assert_eq!(mapped.input, json!({"command": "make build"}));
    }

    #[test]
    fn test_codex_to_claude() {
        let input = json!({"cmd": "cargo test"});
        let mapped = map_tool(
            &AgentKind::Codex,
            &AgentKind::Claude,
            "exec_command",
            &input,
        );
        assert_eq!(mapped.name, "Bash");
        assert_eq!(mapped.input, json!({"command": "cargo test"}));
    }
}
