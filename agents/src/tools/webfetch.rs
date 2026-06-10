use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    let url = string_arg(call, "url");
    if url.is_empty() {
        return failure("webfetch", "url is required".to_string());
    }
    // Real fetch not wired up yet; the v2 dispatcher validates the URL
    // before reaching here. Return a placeholder success so the test path
    // can verify the policy was applied.
    success("webfetch", format!("validated URL: {url}"))
}
