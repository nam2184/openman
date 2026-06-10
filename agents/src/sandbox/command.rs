/// Parse a shell command string into a "prefix" suitable for permission
/// matching. Returns the leading token (command name) and the rest of the
/// command, separated by a space — matching opencode's pattern style.
///
/// Examples:
///   `git status --porcelain` → `git status --porcelain`
///   `cargo test --all`       → `cargo test --all`
///   `echo "hello world"`     → `echo "hello world"`
///   `rm -rf /`               → `rm -rf /`
///   `   ls   -la`            → `ls   -la` (preserves internal whitespace)
pub fn parse_command(command: &str) -> ParsedCommand {
    let trimmed = command.trim_start();
    let first_space = trimmed.find(char::is_whitespace);
    match first_space {
        Some(idx) => ParsedCommand {
            program: trimmed[..idx].to_string(),
            full: trimmed.to_string(),
        },
        None => ParsedCommand {
            program: trimmed.to_string(),
            full: trimmed.to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    /// The first whitespace-delimited token (e.g. `git`, `rm`, `cargo`).
    pub program: String,
    /// The full command, left-trimmed.
    pub full: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let p = parse_command("git status");
        assert_eq!(p.program, "git");
        assert_eq!(p.full, "git status");
    }

    #[test]
    fn parses_command_with_no_args() {
        let p = parse_command("ls");
        assert_eq!(p.program, "ls");
        assert_eq!(p.full, "ls");
    }

    #[test]
    fn parses_command_with_leading_whitespace() {
        let p = parse_command("   cargo test");
        assert_eq!(p.program, "cargo");
        assert_eq!(p.full, "cargo test");
    }

    #[test]
    fn parses_command_with_flags() {
        let p = parse_command("rm -rf /");
        assert_eq!(p.program, "rm");
        assert_eq!(p.full, "rm -rf /");
    }

    #[test]
    fn parses_command_with_quoted_args() {
        let p = parse_command("echo \"hello world\"");
        assert_eq!(p.program, "echo");
        assert_eq!(p.full, "echo \"hello world\"");
    }

    #[test]
    fn parses_empty_command() {
        let p = parse_command("");
        assert_eq!(p.program, "");
        assert_eq!(p.full, "");
    }

    #[test]
    fn parses_whitespace_only_command() {
        let p = parse_command("   ");
        assert_eq!(p.program, "");
        assert_eq!(p.full, "");
    }
}
