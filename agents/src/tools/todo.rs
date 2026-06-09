use crate::{ToolCall, ToolResult};

use super::{string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    success("todo", string_arg(call, "content"))
}
