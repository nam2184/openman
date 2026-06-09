use std::path::Path;

use crate::file_mutation::FileMutationService;
use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    let path = string_arg(call, "path");
    let old = string_arg(call, "old_string");
    let new = string_arg(call, "new_string");

    if old.is_empty() {
        return failure("edit", "old_string is required".to_string());
    }

    let mutation = FileMutationService::new();
    let target = match mutation.target(Path::new(&path)) {
        Ok(target) => target,
        Err(error) => return failure("edit", error.to_string()),
    };
    let original = match std::fs::read(&target.canonical) {
        Ok(content) => content,
        Err(error) => return failure("edit", error.to_string()),
    };
    let content = match String::from_utf8(original.clone()) {
        Ok(content) => content,
        Err(_) => return failure("edit", format!("{path} is not valid UTF-8")),
    };

    if !content.contains(&old) {
        return failure("edit", "old_string was not found".to_string());
    }

    match mutation.write_if_unmodified(&target, &original, content.replacen(&old, &new, 1)) {
        Ok(_) => success("edit", format!("Edited {path}")),
        Err(error) => failure("edit", error.to_string()),
    }
}
