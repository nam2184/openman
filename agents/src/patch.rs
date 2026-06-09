use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hunk {
    Add {
        path: String,
        contents: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        move_path: Option<String>,
        chunks: Vec<UpdateFileChunk>,
    },
}

impl Hunk {
    pub fn path(&self) -> &str {
        match self {
            Hunk::Add { path, .. } => path,
            Hunk::Delete { path } => path,
            Hunk::Update { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFileChunk {
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
    pub change_context: Option<String>,
    pub end_of_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUpdate {
    pub content: String,
    pub bom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    InvalidFormat(String),
    ApplyFailed(String),
}

impl fmt::Display for PatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchError::InvalidFormat(message) => write!(formatter, "{message}"),
            PatchError::ApplyFailed(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for PatchError {}

pub fn parse(patch_text: &str) -> Result<Vec<Hunk>, PatchError> {
    let stripped = strip_heredoc(patch_text.trim());
    let normalized = stripped.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let begin = lines
        .iter()
        .position(|line| line.trim() == "*** Begin Patch");
    let end = lines.iter().position(|line| line.trim() == "*** End Patch");
    let (Some(begin), Some(end)) = (begin, end) else {
        return Err(PatchError::InvalidFormat(
            "Invalid patch format: missing Begin/End markers".to_string(),
        ));
    };
    if begin >= end {
        return Err(PatchError::InvalidFormat(
            "Invalid patch format: missing Begin/End markers".to_string(),
        ));
    }

    let mut hunks = Vec::new();
    let mut index = begin + 1;
    while index < end {
        let line = &lines[index];
        if let Some(path) = line.strip_prefix("*** Add File:") {
            let path = path.trim().to_string();
            if path.is_empty() {
                return Err(PatchError::InvalidFormat(
                    "Invalid add file path".to_string(),
                ));
            }
            let parsed = parse_add(&lines, index + 1)?;
            hunks.push(Hunk::Add {
                path,
                contents: parsed.content,
            });
            index = parsed.next;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File:") {
            let path = path.trim().to_string();
            if path.is_empty() {
                return Err(PatchError::InvalidFormat(
                    "Invalid delete file path".to_string(),
                ));
            }
            hunks.push(Hunk::Delete { path });
            index += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File:") {
            let path = path.trim().to_string();
            if path.is_empty() {
                return Err(PatchError::InvalidFormat(
                    "Invalid update file path".to_string(),
                ));
            }
            let mut next = index + 1;
            let move_path = lines.get(next).and_then(|line| {
                line.strip_prefix("*** Move to:")
                    .map(|path| path.trim().to_string())
            });
            if matches!(move_path.as_deref(), Some("")) {
                return Err(PatchError::InvalidFormat(
                    "Invalid move file path".to_string(),
                ));
            }
            if move_path.is_some() {
                next += 1;
            }
            let parsed = parse_update(&lines, next)?;
            if parsed.chunks.is_empty() {
                return Err(PatchError::InvalidFormat(format!(
                    "Invalid update hunk for {path}: expected at least one @@ chunk"
                )));
            }
            hunks.push(Hunk::Update {
                path,
                move_path,
                chunks: parsed.chunks,
            });
            index = parsed.next;
            continue;
        }
        return Err(PatchError::InvalidFormat(format!(
            "Invalid patch line: {line}"
        )));
    }

    Ok(hunks)
}

pub fn derive(
    path: &str,
    chunks: &[UpdateFileChunk],
    original: &str,
) -> Result<FileUpdate, PatchError> {
    let source = split_bom(original);
    let mut lines = source
        .text
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    let replacements = compute_replacements(&lines, path, chunks)?;
    let mut updated = lines;
    for replacement in replacements.into_iter().rev() {
        updated.splice(
            replacement.start..replacement.start + replacement.remove,
            replacement.insert,
        );
    }
    if !updated.last().is_some_and(String::is_empty) {
        updated.push(String::new());
    }
    let next = split_bom(&updated.join("\n"));

    Ok(FileUpdate {
        content: next.text,
        bom: source.bom || next.bom,
    })
}

pub fn join_bom(text: &str, bom: bool) -> String {
    let stripped = split_bom(text).text;
    if bom {
        format!("\u{FEFF}{stripped}")
    } else {
        stripped
    }
}

struct ParsedAdd {
    content: String,
    next: usize,
}

struct ParsedUpdate {
    chunks: Vec<UpdateFileChunk>,
    next: usize,
}

struct Replacement {
    start: usize,
    remove: usize,
    insert: Vec<String>,
}

struct SplitBom {
    bom: bool,
    text: String,
}

fn parse_add(lines: &[String], start: usize) -> Result<ParsedAdd, PatchError> {
    let mut content = Vec::new();
    let mut index = start;
    while index < lines.len() && !lines[index].starts_with("***") {
        let Some(line) = lines[index].strip_prefix('+') else {
            return Err(PatchError::InvalidFormat(format!(
                "Invalid add file line: {}",
                lines[index]
            )));
        };
        content.push(line.to_string());
        index += 1;
    }

    Ok(ParsedAdd {
        content: content.join("\n"),
        next: index,
    })
}

fn parse_update(lines: &[String], start: usize) -> Result<ParsedUpdate, PatchError> {
    let mut chunks = Vec::new();
    let mut index = start;
    while index < lines.len() && !lines[index].starts_with("***") {
        if !lines[index].starts_with("@@") {
            return Err(PatchError::InvalidFormat(format!(
                "Invalid update file line: {}",
                lines[index]
            )));
        }
        let change_context = lines[index]
            .strip_prefix("@@")
            .unwrap_or_default()
            .trim()
            .to_string();
        let change_context = (!change_context.is_empty()).then_some(change_context);
        let mut old_lines = Vec::new();
        let mut new_lines = Vec::new();
        let mut end_of_file = false;
        index += 1;
        while index < lines.len() && !lines[index].starts_with("@@") {
            let line = &lines[index];
            if line == "*** End of File" {
                end_of_file = true;
                index += 1;
                break;
            }
            if line.starts_with("***") {
                break;
            }
            if let Some(value) = line.strip_prefix(' ') {
                old_lines.push(value.to_string());
                new_lines.push(value.to_string());
            } else if let Some(value) = line.strip_prefix('-') {
                old_lines.push(value.to_string());
            } else if let Some(value) = line.strip_prefix('+') {
                new_lines.push(value.to_string());
            } else {
                return Err(PatchError::InvalidFormat(format!(
                    "Invalid update chunk line: {line}"
                )));
            }
            index += 1;
        }
        chunks.push(UpdateFileChunk {
            old_lines,
            new_lines,
            change_context,
            end_of_file,
        });
    }

    Ok(ParsedUpdate {
        chunks,
        next: index,
    })
}

fn compute_replacements(
    lines: &[String],
    path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<Replacement>, PatchError> {
    let mut replacements = Vec::new();
    let mut line_index = 0;
    for chunk in chunks {
        if let Some(change_context) = &chunk.change_context {
            let context = seek(lines, &[change_context.to_string()], line_index, false);
            let Some(context) = context else {
                return Err(PatchError::ApplyFailed(format!(
                    "Failed to find context '{change_context}' in {path}"
                )));
            };
            line_index = context + 1;
        }
        if chunk.old_lines.is_empty() {
            replacements.push(Replacement {
                start: lines.len(),
                remove: 0,
                insert: chunk.new_lines.clone(),
            });
            continue;
        }

        let mut old_lines = chunk.old_lines.clone();
        let mut new_lines = chunk.new_lines.clone();
        let mut found = seek(lines, &old_lines, line_index, chunk.end_of_file);
        if found.is_none() && old_lines.last().is_some_and(String::is_empty) {
            old_lines.pop();
            if new_lines.last().is_some_and(String::is_empty) {
                new_lines.pop();
            }
            found = seek(lines, &old_lines, line_index, chunk.end_of_file);
        }
        let Some(found) = found else {
            return Err(PatchError::ApplyFailed(format!(
                "Failed to find expected lines in {path}:\n{}",
                chunk.old_lines.join("\n")
            )));
        };
        replacements.push(Replacement {
            start: found,
            remove: old_lines.len(),
            insert: new_lines,
        });
        line_index = found + old_lines.len();
    }
    replacements.sort_by_key(|replacement| replacement.start);
    Ok(replacements)
}

fn seek(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > lines.len() {
        return None;
    }
    for compare in [exact, rstrip, trim, normalized] {
        if eof {
            let offset = lines.len().saturating_sub(pattern.len());
            if offset >= start && matches_at(lines, pattern, offset, compare) {
                return Some(offset);
            }
        }
        for offset in start..=lines.len() - pattern.len() {
            if matches_at(lines, pattern, offset, compare) {
                return Some(offset);
            }
        }
    }
    None
}

fn matches_at(
    lines: &[String],
    pattern: &[String],
    offset: usize,
    compare: fn(&str, &str) -> bool,
) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(index, line)| compare(&lines[offset + index], line))
}

fn exact(left: &str, right: &str) -> bool {
    left == right
}

fn rstrip(left: &str, right: &str) -> bool {
    left.trim_end() == right.trim_end()
}

fn trim(left: &str, right: &str) -> bool {
    left.trim() == right.trim()
}

fn normalized(left: &str, right: &str) -> bool {
    normalize(left.trim()) == normalize(right.trim())
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => "'".chars().collect::<Vec<_>>(),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => "\"".chars().collect::<Vec<_>>(),
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' => {
                "-".chars().collect::<Vec<_>>()
            }
            '\u{2026}' => "...".chars().collect::<Vec<_>>(),
            '\u{00A0}' => " ".chars().collect::<Vec<_>>(),
            _ => vec![character],
        })
        .collect()
}

fn split_bom(text: &str) -> SplitBom {
    let stripped = text.trim_start_matches('\u{FEFF}');
    SplitBom {
        bom: stripped.len() != text.len(),
        text: stripped.to_string(),
    }
}

fn strip_heredoc(input: &str) -> String {
    let Some(first_line_end) = input.find('\n') else {
        return input.to_string();
    };
    let first_line = input[..first_line_end].trim();
    let marker = first_line
        .strip_prefix("cat ")
        .unwrap_or(first_line)
        .strip_prefix("<<")
        .map(str::trim)
        .map(|marker| marker.trim_matches('\''))
        .map(|marker| marker.trim_matches('"'));
    let Some(marker) = marker else {
        return input.to_string();
    };
    if marker.is_empty()
        || !marker
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return input.to_string();
    }
    let body = &input[first_line_end + 1..];
    let Some(last_line_start) = body.rfind('\n') else {
        return input.to_string();
    };
    if body[last_line_start + 1..].trim() == marker {
        body[..last_line_start].to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{derive, join_bom, parse, Hunk};

    #[test]
    fn parses_add_hunk() {
        let hunks =
            parse("*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch").unwrap();
        assert_eq!(
            hunks,
            vec![Hunk::Add {
                path: "hello.txt".to_string(),
                contents: "hello".to_string(),
            }]
        );
    }

    #[test]
    fn derives_update_with_trim_fallback() {
        let hunks = parse(
            "*** Begin Patch\n*** Update File: hello.txt\n@@\n-  hello  \n+  goodbye  \n*** End Patch",
        )
        .unwrap();
        let Hunk::Update { chunks, .. } = &hunks[0] else {
            panic!("expected update hunk");
        };
        let update = derive("hello.txt", chunks, "hello\n").unwrap();
        assert_eq!(update.content, "  goodbye  \n");
    }

    #[test]
    fn preserves_bom() {
        let update = join_bom("hello", true);
        assert!(update.starts_with('\u{FEFF}'));
    }
}
