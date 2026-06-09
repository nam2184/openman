use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success, wildcard_match};

pub fn run(call: &ToolCall) -> ToolResult {
    let root = string_arg(call, "path");
    let root_path = std::path::PathBuf::from(&root);
    let pattern = string_arg(call, "pattern");
    let pattern = if pattern.is_empty() {
        "*".to_string()
    } else {
        pattern
    };

    let mut matches = Vec::new();
    for entry in walkdir::WalkDir::new(&root_path).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&root_path)
            .unwrap_or(entry.path())
            .to_string_lossy();
        if wildcard_match(&pattern, &relative)
            || wildcard_match(&pattern, &entry.file_name().to_string_lossy())
        {
            matches.push(entry.path().to_string_lossy().to_string());
        }
    }

    if matches.is_empty() {
        return failure("glob", "No files found".to_string());
    }
    success("glob", matches.join("\n"))
}
