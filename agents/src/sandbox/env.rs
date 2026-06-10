use std::collections::HashSet;
use std::ffi::OsString;

/// Returns a copy of `env` with sensitive-looking variables removed.
///
/// We strip:
/// - Variables whose name contains KEY, SECRET, TOKEN, PASSWORD, CREDENTIAL
/// - AWS_* (access keys)
/// - GITHUB_TOKEN, GITLAB_TOKEN, etc.
pub fn scrub_env(env: &[(OsString, OsString)]) -> Vec<(OsString, OsString)> {
    env.iter()
        .filter(|(key, _)| !is_sensitive_key(key))
        .cloned()
        .collect()
}

fn is_sensitive_key(key: &std::ffi::OsStr) -> bool {
    let upper = key.to_string_lossy().to_uppercase();
    for needle in SENSITIVE_NEEDLES {
        if upper.contains(needle) {
            return true;
        }
    }
    false
}

const SENSITIVE_NEEDLES: &[&str] = &[
    "KEY",
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "CREDENTIAL",
    "AWS_",
    "PRIVATE_KEY",
];

/// Variables that are commonly needed by build tools. These are *kept* even
/// if a more general scrubber would remove them. Add to this set when a
/// build/test command needs the variable and it's not actually a secret.
pub fn keep_list() -> HashSet<&'static str> {
    [
        "PATH",
        "HOME",
        "USER",
        "SHELL",
        "LANG",
        "LC_ALL",
        "PWD",
        "TMPDIR",
        "TERM",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "NODE_PATH",
        "NODE_ENV",
        "GOPATH",
        "JAVA_HOME",
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn k(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn removes_api_keys() {
        let env = vec![
            (k("PATH"), k("/usr/bin")),
            (k("OPENAI_API_KEY"), k("sk-xxx")),
            (k("ANTHROPIC_API_KEY"), k("sk-xxx")),
        ];
        let scrubbed = scrub_env(&env);
        assert_eq!(scrubbed.len(), 1);
        assert_eq!(scrubbed[0].0, k("PATH"));
    }

    #[test]
    fn removes_secrets_tokens_passwords() {
        let env = vec![
            (k("PATH"), k("/usr/bin")),
            (k("DB_PASSWORD"), k("hunter2")),
            (k("GITHUB_TOKEN"), k("ghp_xxx")),
            (k("GITLAB_TOKEN"), k("xxx")),
            (k("JWT_SECRET"), k("xxx")),
            (k("AWS_ACCESS_KEY_ID"), k("xxx")),
            (k("AWS_SECRET_ACCESS_KEY"), k("xxx")),
        ];
        let scrubbed = scrub_env(&env);
        let names: Vec<String> = scrubbed.iter().map(|(k, _)| k.to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["PATH".to_string()]);
    }

    #[test]
    fn keeps_non_sensitive_variables() {
        let env = vec![
            (k("PATH"), k("/usr/bin")),
            (k("HOME"), k("/home/user")),
            (k("USER"), k("user")),
            (k("LANG"), k("en_US.UTF-8")),
            (k("CARGO_HOME"), k("/home/user/.cargo")),
        ];
        let scrubbed = scrub_env(&env);
        assert_eq!(scrubbed.len(), 5);
    }

    #[test]
    fn matches_substring_case_insensitively() {
        let env = vec![(k("my_secret_value"), k("xxx"))];
        let scrubbed = scrub_env(&env);
        assert!(scrubbed.is_empty());
    }

    #[test]
    fn matches_token_substring() {
        let env = vec![(k("NOTIFICATION_TOKEN"), k("xxx"))];
        let scrubbed = scrub_env(&env);
        assert!(scrubbed.is_empty());
    }

    #[test]
    fn keep_list_contains_path() {
        assert!(keep_list().contains("PATH"));
    }
}
