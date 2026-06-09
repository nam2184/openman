pub mod apply_patch;
pub mod edit;
pub mod external_directory;
pub mod glob;
pub mod grep;
pub mod invalid;
pub mod lsp;
pub mod plan;
pub mod question;
pub mod read;
pub mod shell;
pub mod skill;
pub mod task;
pub mod todo;
pub mod webfetch;
pub mod websearch;
pub mod write;

use crate::permission::{PermissionMode, PermissionService};
use crate::{Tool, ToolCall, ToolResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolContext {
    pub mode: PermissionMode,
}

impl ToolContext {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new(PermissionMode::Build)
    }
}

pub fn default_tools() -> Vec<Tool> {
    vec![
        Tool::new("read", "Read a file from disk"),
        Tool::new("write", "Write content to a file"),
        Tool::new("edit", "Replace text in an existing file"),
        Tool::new("apply_patch", "Apply a file-oriented patch"),
        Tool::new("glob", "Find files by glob-like pattern"),
        Tool::new("grep", "Search file contents"),
        Tool::new("shell", "Run a shell command"),
        Tool::new("task", "Run a subagent task"),
        Tool::new("skill", "Load a skill by name"),
        Tool::new("todo", "Update the session todo list"),
        Tool::new("question", "Ask the user a structured question"),
        Tool::new("webfetch", "Fetch a web URL"),
        Tool::new("websearch", "Search the web"),
        Tool::new("lsp", "Query language-server context"),
        Tool::new("plan", "Enter or exit planning mode"),
        Tool::new("external_directory", "Request external directory access"),
        Tool::new("invalid", "Report invalid tool calls"),
    ]
}

pub fn run_tool(call: &ToolCall) -> ToolResult {
    run_tool_with_context(call, &ToolContext::default())
}

pub fn run_tool_with_mode(call: &ToolCall, mode: PermissionMode) -> ToolResult {
    run_tool_with_context(call, &ToolContext::new(mode))
}

pub fn run_tool_with_context(call: &ToolCall, context: &ToolContext) -> ToolResult {
    if let Err(error) = PermissionService::new(context.mode).assert_tool_call(call) {
        return failure(&call.name, error.to_string());
    }

    match call.name.as_str() {
        "read" | "read_file" => read::run(call),
        "write" | "write_file" => write::run(call),
        "edit" => edit::run(call),
        "apply_patch" => apply_patch::run(call),
        "glob" | "search_files" => glob::run(call),
        "grep" => grep::run(call),
        "shell" | "bash" => shell::run(call),
        "task" => task::run(call),
        "skill" => skill::run(call),
        "todo" | "todowrite" => todo::run(call),
        "question" => question::run(call),
        "webfetch" => webfetch::run(call),
        "websearch" => websearch::run(call),
        "lsp" => lsp::run(call),
        "plan" => plan::run(call),
        "external_directory" => external_directory::run(call),
        "invalid" => invalid::run(call),
        _ => invalid::unknown(call),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use crate::permission::PermissionMode;
    use crate::ToolCall;

    use super::run_tool_with_mode;

    #[test]
    fn plan_mode_blocks_write_before_file_mutation() {
        let path = std::env::temp_dir().join(format!(
            "openman-plan-deny-{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let result = run_tool_with_mode(
            &call(
                "write",
                &[
                    ("path", path.to_string_lossy().as_ref()),
                    ("content", "blocked"),
                ],
            ),
            PermissionMode::Plan,
        );

        assert!(!result.success);
        assert!(!path.exists());
    }

    #[test]
    fn build_mode_allows_write() {
        let path = std::env::temp_dir().join(format!(
            "openman-build-allow-{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let result = run_tool_with_mode(
            &call(
                "write",
                &[
                    ("path", path.to_string_lossy().as_ref()),
                    ("content", "allowed"),
                ],
            ),
            PermissionMode::Build,
        );

        assert!(result.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "allowed");
        let _ = fs::remove_file(path);
    }

    fn call(name: &str, args: &[(&str, &str)]) -> ToolCall {
        ToolCall {
            name: name.to_string(),
            arguments: args
                .iter()
                .map(|(key, value)| (key.to_string(), json!(value)))
                .collect::<HashMap<_, _>>(),
        }
    }
}

pub(crate) fn string_arg(call: &ToolCall, key: &str) -> String {
    call.arguments
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn usize_arg(call: &ToolCall, key: &str) -> Option<usize> {
    call.arguments
        .get(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
}

pub(crate) fn success(tool: &str, output: String) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        success: true,
        output,
        error: None,
    }
}

pub(crate) fn failure(tool: &str, error: String) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        success: false,
        output: String::new(),
        error: Some(error),
    }
}

pub(crate) fn not_implemented(tool: &str, detail: &str) -> ToolResult {
    failure(
        tool,
        format!("{tool} requires runtime integration: {detail}"),
    )
}

pub(crate) fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if !pattern.contains('*') {
        return value.contains(pattern);
    }

    let mut remaining = value;
    for part in pattern.split('*').filter(|part| !part.is_empty()) {
        if let Some(index) = remaining.find(part) {
            remaining = &remaining[index + part.len()..];
        } else {
            return false;
        }
    }
    true
}
