use std::process::Command;

use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success};

pub fn run(call: &ToolCall) -> ToolResult {
    let command = string_arg(call, "command");
    if command.is_empty() {
        return failure("shell", "command is required".to_string());
    }

    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &command]);
        cmd
    };

    let workdir = string_arg(call, "workdir");
    if !workdir.is_empty() {
        cmd.current_dir(workdir);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            success("shell", String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => {
            let bytes: &[u8] = if output.stderr.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            };
            failure("shell", String::from_utf8_lossy(bytes).to_string())
        }
        Err(error) => failure("shell", error.to_string()),
    }
}
