use std::path::Path;

use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success, usize_arg};

pub fn run(call: &ToolCall) -> ToolResult {
    let path = string_arg(call, "path");
    let offset = usize_arg(call, "offset").unwrap_or(1).max(1);
    let limit = usize_arg(call, "limit");

    match std::fs::read_to_string(Path::new(&path)) {
        Ok(content) => success("read", format_lines(&content, offset, limit)),
        Err(error) => failure("read", error.to_string()),
    }
}

fn format_lines(content: &str, offset: usize, limit: Option<usize>) -> String {
    content
        .lines()
        .enumerate()
        .skip(offset.saturating_sub(1))
        .take(limit.unwrap_or(usize::MAX))
        .map(|(index, line)| format!("{}: {}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}
