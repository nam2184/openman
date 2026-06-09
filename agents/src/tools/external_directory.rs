use crate::{ToolCall, ToolResult};

use super::not_implemented;

pub fn run(_call: &ToolCall) -> ToolResult {
    not_implemented(
        "external_directory",
        "requires client permission and workspace policy integration",
    )
}
