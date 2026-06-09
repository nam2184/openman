use crate::{ToolCall, ToolResult};

use super::not_implemented;

pub fn run(_call: &ToolCall) -> ToolResult {
    not_implemented("lsp", "requires language-server lifecycle management")
}
