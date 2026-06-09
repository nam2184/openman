use crate::{ToolCall, ToolResult};

use super::failure;

pub fn run(call: &ToolCall) -> ToolResult {
    failure("invalid", format!("Invalid tool call: {}", call.name))
}

pub fn unknown(call: &ToolCall) -> ToolResult {
    failure(&call.name, "Unknown tool".to_string())
}
