use crate::{ToolCall, ToolResult};

use super::{string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    success("plan", string_arg(call, "mode"))
}
