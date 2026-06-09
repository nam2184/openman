use std::path::Path;

use crate::file_mutation::FileMutationService;
use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    let path = string_arg(call, "path");
    let content = string_arg(call, "content");
    let mutation = FileMutationService::new();
    let target = match mutation.target(Path::new(&path)) {
        Ok(target) => target,
        Err(error) => return failure("write", error.to_string()),
    };

    match mutation.write_text_preserving_bom(&target, &content) {
        Ok(_) => success("write", format!("Wrote {path}")),
        Err(error) => failure("write", error.to_string()),
    }
}
