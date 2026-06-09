use crate::{ToolCall, ToolResult};

use super::not_implemented;

pub fn run(_call: &ToolCall) -> ToolResult {
    not_implemented(
        "task",
        "requires subagent scheduler and session execution context",
    )
}
