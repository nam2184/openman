use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathContainmentError {
    /// Path tries to escape the allowed root (via `..` or absolute path).
    EscapesRoot { path: PathBuf, root: PathBuf },
    /// Path is outside the project root and external_directory isn't allowed
    /// for this prefix.
    ExternalAccess { path: PathBuf },
    /// Path resolved to a symlink that points outside the root.
    SymlinkEscape { path: PathBuf, target: PathBuf },
    /// Path is empty or otherwise unusable.
    InvalidPath { path: PathBuf },
}

impl std::fmt::Display for PathContainmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EscapesRoot { path, root } => {
                write!(f, "path '{}' escapes root '{}'", path.display(), root.display())
            }
            Self::ExternalAccess { path } => {
                write!(f, "path '{}' is outside the project root", path.display())
            }
            Self::SymlinkEscape { path, target } => write!(
                f,
                "symlink '{}' points outside the root: '{}'",
                path.display(),
                target.display()
            ),
            Self::InvalidPath { path } => write!(f, "invalid path: '{}'", path.display()),
        }
    }
}

impl std::error::Error for PathContainmentError {}

/// A policy describing which paths a tool may access. Created per-session from
/// the project root plus any `external_directory` allowlist.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Canonical project root. All paths must resolve to be inside this,
    /// unless they match an `external_directory` rule.
    pub project_root: PathBuf,
    /// Additional allowed path prefixes (each must be canonical).
    pub external_roots: Vec<PathBuf>,
}

impl SandboxPolicy {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            external_roots: Vec::new(),
        }
    }

    pub fn with_external(mut self, path: impl Into<PathBuf>) -> Self {
        self.external_roots.push(path.into());
        self
    }

    /// Resolve a user-supplied path into its canonical form and check it
    /// against the policy. Returns the canonical path on success.
    pub fn resolve(&self, path: impl AsRef<Path>) -> Result<PathBuf, PathContainmentError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(PathContainmentError::InvalidPath { path: path.to_path_buf() });
        }

        // First, normalize the literal path (resolving `..` and `.` syntactically).
        // This catches obvious escape attempts before we touch the filesystem.
        let normalized = normalize_path(path);

        // If the path is relative, anchor it to the project root.
        let absolute = if normalized.is_absolute() {
            normalized
        } else {
            self.project_root.join(normalized)
        };

        // Check containment: does the path fall within the project root or any
        // external root?
        if !self.is_allowed(&absolute) {
            return Err(PathContainmentError::ExternalAccess { path: absolute });
        }

        // Try to canonicalize. If the file doesn't exist yet (e.g., for a
        // write), fall back to the normalized path.
        let canonical = absolute.canonicalize().unwrap_or(absolute);

        // Re-check after canonicalization: symlinks could point outside.
        if !self.is_allowed(&canonical) {
            return Err(PathContainmentError::ExternalAccess { path: canonical });
        }

        Ok(canonical)
    }

    fn is_allowed(&self, path: &Path) -> bool {
        if path.starts_with(&self.project_root) {
            return true;
        }
        self.external_roots.iter().any(|root| path.starts_with(root))
    }
}

/// Normalize a path syntactically: collapse `.` and `..` components without
/// touching the filesystem. Does not resolve symlinks.
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let absolute = path.is_absolute();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Don't pop past a root component.
                if !out.pop() && !absolute {
                    out.push("..");
                }
            }
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::Normal(c) => out.push(c),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_returns_canonical_path_inside_root() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hi").unwrap();

        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let resolved = policy.resolve(&file).unwrap();
        // The resolved path may differ from `file` due to symlink resolution
        // (e.g., on macOS tempdir is a symlink).
        assert!(resolved.is_absolute());
        assert!(policy.is_allowed(&resolved));
    }

    #[test]
    fn resolve_rejects_dotdot_escape() {
        let dir = TempDir::new().unwrap();
        let escape = dir.path().join("..").join("etc").join("passwd");
        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let result = policy.resolve(&escape);
        assert!(matches!(result, Err(PathContainmentError::ExternalAccess { .. })));
    }

    #[test]
    fn resolve_rejects_absolute_path_outside_root() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let result = policy.resolve("/etc/passwd");
        assert!(matches!(result, Err(PathContainmentError::ExternalAccess { .. })));
    }

    #[test]
    fn resolve_rejects_empty_path() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let result = policy.resolve("");
        assert!(matches!(result, Err(PathContainmentError::InvalidPath { .. })));
    }

    #[test]
    fn resolve_allows_external_root() {
        let project = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();
        let file = external.path().join("outside.txt");
        std::fs::write(&file, "external").unwrap();

        let policy = SandboxPolicy::new(project.path().to_path_buf())
            .with_external(external.path().to_path_buf());
        let resolved = policy.resolve(&file).unwrap();
        assert!(resolved.starts_with(external.path()));
    }

    #[test]
    fn resolve_rejects_path_in_unrelated_directory() {
        let project = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let file = other.path().join("not-allowed.txt");
        std::fs::write(&file, "nope").unwrap();

        let policy = SandboxPolicy::new(project.path().to_path_buf());
        let result = policy.resolve(&file);
        assert!(matches!(result, Err(PathContainmentError::ExternalAccess { .. })));
    }

    #[test]
    fn resolve_handles_relative_path_inside_root() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let resolved = policy.resolve("a.txt").unwrap();
        assert!(policy.is_allowed(&resolved));
    }

    #[test]
    fn resolve_creates_nonexistent_paths_normally() {
        // For paths that don't exist yet (e.g. a write target), the policy
        // should still validate the prefix containment.
        let dir = TempDir::new().unwrap();
        let new_file = dir.path().join("new.txt");
        let policy = SandboxPolicy::new(dir.path().to_path_buf());
        let resolved = policy.resolve(&new_file).unwrap();
        assert!(resolved.starts_with(dir.path()));
    }

    #[test]
    fn normalize_collapses_dot() {
        assert_eq!(normalize_path(Path::new("./a/./b")), Path::new("a/b"));
    }

    #[test]
    fn normalize_handles_dotdot() {
        assert_eq!(normalize_path(Path::new("a/../b")), Path::new("b"));
    }

    #[test]
    fn normalize_does_not_escape_absolute_root() {
        assert_eq!(normalize_path(Path::new("/../etc")), Path::new("/etc"));
    }

    #[test]
    fn normalize_relative_dotdot_kept() {
        // Relative paths that go above the implicit start keep `..` (we have
        // nothing to pop).
        let result = normalize_path(Path::new("../a"));
        assert_eq!(result, Path::new("../a"));
    }
}
