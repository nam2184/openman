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
    let include = string_arg(call, "include");

    if pattern.is_empty() {
        return failure("grep", "pattern is required".to_string());
    }

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
        if !include.is_empty() && !wildcard_match(&include, &entry.file_name().to_string_lossy()) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (index, line) in content.lines().enumerate() {
            if line.contains(&pattern) {
                matches.push(format!(
                    "{}:{}: {}",
                    entry.path().display(),
                    index + 1,
                    line
                ));
            }
        }
    }

    if matches.is_empty() {
        return failure("grep", "No matches found".to_string());
    }
    success("grep", matches.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn call(pattern: &str) -> ToolCall {
        ToolCall {
            name: "grep".to_string(),
            arguments: HashMap::from([("pattern".to_string(), json!(pattern))]),
        }
    }

    #[test]
    fn run_with_root_finds_matching_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello world\nfoo bar\n").unwrap();
        let result = run_with_root(&call("hello"), dir.path());
        assert!(result.success);
        assert!(result.output.contains("hello world"));
    }

    #[test]
    fn run_with_root_no_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let result = run_with_root(&call("nope"), dir.path());
        assert!(!result.success);
    }

    #[test]
    fn run_with_root_requires_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = call("anything");
        c.arguments.remove("pattern");
        let result = run_with_root(&c, dir.path());
        assert!(!result.success);
    }
}
