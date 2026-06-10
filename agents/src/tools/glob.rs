use std::path::{Path, PathBuf};

use crate::{ToolCall, ToolResult};

use super::{failure, string_arg, success, wildcard_match};

pub fn run(call: &ToolCall) -> ToolResult {
    let root = string_arg(call, "path");
    let root_path = if root.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&root)
    };
    run_with_root(call, &root_path)
}

pub fn run_with_root(call: &ToolCall, root: &Path) -> ToolResult {
    let pattern = string_arg(call, "pattern");
    let pattern = if pattern.is_empty() {
        "*".to_string()
    } else {
        pattern
    };

    let mut matches = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(20)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy();
        if wildcard_match(&pattern, &relative)
            || wildcard_match(&pattern, &entry.file_name().to_string_lossy())
        {
            matches.push(entry.path().to_string_lossy().to_string());
        }
    }

    if matches.is_empty() {
        return failure("glob", "No files found".to_string());
    }
    success("glob", matches.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::json;

    fn call(pattern: &str) -> ToolCall {
        ToolCall {
            name: "glob".to_string(),
            arguments: HashMap::from([("pattern".to_string(), json!(pattern))]),
        }
    }

    #[test]
    fn run_with_root_finds_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.rs"), "b").unwrap();
        let result = run_with_root(&call("*.txt"), dir.path());
        assert!(result.success);
        assert!(result.output.contains("a.txt"));
        assert!(!result.output.contains("b.rs"));
    }

    #[test]
    fn run_with_root_reports_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_with_root(&call("*.zzz"), dir.path());
        assert!(!result.success);
    }
}

