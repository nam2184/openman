use crate::{ToolCall, ToolResult};

use super::not_implemented;

pub fn run(_call: &ToolCall) -> ToolResult {
    not_implemented(
        "webfetch",
        "requires HTTP client/runtime policy integration",
    )
}
