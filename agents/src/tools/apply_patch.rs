use std::path::Path;

use crate::file_mutation::{FileMutationError, FileMutationService, FileTarget};
use crate::patch::{self, Hunk};
use crate::{ToolCall, ToolResult};

use super::{failure, success};

pub fn run(call: &ToolCall) -> ToolResult {
    let patch_text = patch_text_arg(call);
    if patch_text.trim().is_empty() {
        return failure("apply_patch", "patchText is required".to_string());
    }

    match apply_patch(&patch_text) {
        Ok(applied) => success("apply_patch", model_output(&applied)),
        Err(error) => failure("apply_patch", error),
    }
}

pub fn apply_patch(patch_text: &str) -> Result<Vec<AppliedPatch>, String> {
    let hunks = patch::parse(patch_text)
        .map_err(|error| format!("apply_patch verification failed: {error}"))?;
    if hunks.is_empty() {
        return Err("patch rejected: empty patch".to_string());
    }
    if hunks.iter().any(|hunk| {
        matches!(
            hunk,
            Hunk::Update {
                move_path: Some(_),
                ..
            }
        )
    }) {
        return Err("apply_patch moves are not supported yet".to_string());
    }

    let mutation = FileMutationService::new();
    let mut prepared = Vec::new();
    for hunk in hunks {
        prepared.push(prepare_hunk(&mutation, hunk)?);
    }

    let mut applied = Vec::new();
    for change in prepared {
        let result = apply_prepared(&mutation, &change)
            .map_err(|error| partial_error(&applied, change.path(), error))?;
        applied.push(result);
    }

    Ok(applied)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPatch {
    pub kind: AppliedPatchKind,
    pub resource: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedPatchKind {
    Add,
    Update,
    Delete,
}

enum PreparedPatch {
    Add {
        target: FileTarget,
        content: Vec<u8>,
    },
    Update {
        path: String,
        target: FileTarget,
        expected: Vec<u8>,
        content: Vec<u8>,
    },
    Delete {
        target: FileTarget,
    },
}

impl PreparedPatch {
    fn path(&self) -> &str {
        match self {
            PreparedPatch::Add { target, .. } => &target.resource,
            PreparedPatch::Update { path, .. } => path,
            PreparedPatch::Delete { target } => &target.resource,
        }
    }
}

fn patch_text_arg(call: &ToolCall) -> String {
    for key in ["patchText", "patch_text", "patch"] {
        if let Some(value) = call.arguments.get(key).and_then(|value| value.as_str()) {
            return value.to_string();
        }
    }
    String::new()
}

fn prepare_hunk(mutation: &FileMutationService, hunk: Hunk) -> Result<PreparedPatch, String> {
    match hunk {
        Hunk::Add { path, contents } => {
            let target = mutation
                .target(Path::new(&path))
                .map_err(|error| error.to_string())?;
            let content = if contents.ends_with('\n') || contents.is_empty() {
                contents
            } else {
                format!("{contents}\n")
            };
            Ok(PreparedPatch::Add {
                target,
                content: content.into_bytes(),
            })
        }
        Hunk::Delete { path } => {
            let target = mutation
                .target(Path::new(&path))
                .map_err(|error| error.to_string())?;
            require_file(&target)?;
            Ok(PreparedPatch::Delete { target })
        }
        Hunk::Update { path, chunks, .. } => {
            let target = mutation
                .target(Path::new(&path))
                .map_err(|error| error.to_string())?;
            require_file(&target)?;
            let expected = std::fs::read(&target.canonical).map_err(|error| error.to_string())?;
            let original = String::from_utf8(expected.clone())
                .map_err(|_| format!("{} is not valid UTF-8", target.resource))?;
            let update =
                patch::derive(&path, &chunks, &original).map_err(|error| error.to_string())?;
            Ok(PreparedPatch::Update {
                path,
                target,
                expected,
                content: patch::join_bom(&update.content, update.bom).into_bytes(),
            })
        }
    }
}

fn apply_prepared(
    mutation: &FileMutationService,
    change: &PreparedPatch,
) -> Result<AppliedPatch, FileMutationError> {
    match change {
        PreparedPatch::Add { target, content } => {
            let result = mutation.create(target, content)?;
            Ok(applied(
                AppliedPatchKind::Add,
                result.resource,
                result.target,
            ))
        }
        PreparedPatch::Update {
            target,
            expected,
            content,
            ..
        } => {
            let result = mutation.write_if_unmodified(target, expected, content)?;
            Ok(applied(
                AppliedPatchKind::Update,
                result.resource,
                result.target,
            ))
        }
        PreparedPatch::Delete { target } => {
            let result = mutation.remove(target)?;
            Ok(applied(
                AppliedPatchKind::Delete,
                result.resource,
                result.target,
            ))
        }
    }
}

fn require_file(target: &FileTarget) -> Result<(), String> {
    let metadata = std::fs::metadata(&target.canonical).map_err(|error| error.to_string())?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(format!("{} is not a file", target.resource))
    }
}

fn applied(kind: AppliedPatchKind, resource: String, target: std::path::PathBuf) -> AppliedPatch {
    AppliedPatch {
        kind,
        resource,
        target: target.to_string_lossy().to_string(),
    }
}

fn partial_error(applied: &[AppliedPatch], path: &str, error: FileMutationError) -> String {
    if applied.is_empty() {
        return format!("Unable to apply patch at {path}: {error}");
    }
    format!(
        "Patch partially applied before failing at {path}: {error}. Applied: {}",
        applied
            .iter()
            .map(|item| item.resource.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn model_output(applied: &[AppliedPatch]) -> String {
    let mut lines = vec!["Applied patch sequentially:".to_string()];
    lines.extend(applied.iter().map(|item| {
        let status = match item.kind {
            AppliedPatchKind::Add => "A",
            AppliedPatchKind::Update => "M",
            AppliedPatchKind::Delete => "D",
        };
        format!("{status} {}", item.resource)
    }));
    lines.join("\n")
}
