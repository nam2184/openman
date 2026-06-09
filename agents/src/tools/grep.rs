use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success, wildcard_match};

pub fn run(call: &ToolCall) -> ToolResult {
    let root = string_arg(call, "path");
    let pattern = string_arg(call, "pattern");
    let include = string_arg(call, "include");

    if pattern.is_empty() {
        return failure("grep", "pattern is required".to_string());
    }

    let mut matches = Vec::new();
    for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if !include.is_empty() && !wildcard_match(&include, &entry.file_name().to_string_lossy()) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (index, line) in content.lines().enumerate() {
            if line.contains(&pattern) {
                matches.push(format!(
                    "{}:{}: {}",
                    entry.path().display(),
                    index + 1,
                    line
                ));
            }
        }
    }

    if matches.is_empty() {
        return failure("grep", "No matches found".to_string());
    }
    success("grep", matches.join("\n"))
}
