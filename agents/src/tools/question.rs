use crate::{ToolCall, ToolResult};

use super::not_implemented;

pub fn run(_call: &ToolCall) -> ToolResult {
    not_implemented("question", "requires an interactive client bridge")
}
