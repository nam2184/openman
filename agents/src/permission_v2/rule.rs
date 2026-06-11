use serde::{Deserialize, Serialize};

/// What to do when a permission rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    Allow,
    Ask,
    Deny,
}

impl PermissionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionAction::Allow => "allow",
            PermissionAction::Ask => "ask",
            PermissionAction::Deny => "deny",
        }
    }
}

impl Default for PermissionAction {
    fn default() -> Self {
        PermissionAction::Ask
    }
}

/// A single permission rule: applies to a `permission` (tool name or category)
/// and a `pattern` (e.g. a file path, a command, a URL). Last matching rule wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: PermissionAction,
}

impl PermissionRule {
    pub fn new(
        permission: impl Into<String>,
        pattern: impl Into<String>,
        action: PermissionAction,
    ) -> Self {
        Self {
            permission: permission.into(),
            pattern: pattern.into(),
            action,
        }
    }

    pub fn allow(permission: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::new(permission, pattern, PermissionAction::Allow)
    }

    pub fn ask(permission: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::new(permission, pattern, PermissionAction::Ask)
    }

    pub fn deny(permission: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::new(permission, pattern, PermissionAction::Deny)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_serde_round_trip() {
        for action in [
            PermissionAction::Allow,
            PermissionAction::Ask,
            PermissionAction::Deny,
        ] {
            let s = serde_json::to_string(&action).unwrap();
            let back: PermissionAction = serde_json::from_str(&s).unwrap();
            assert_eq!(action, back);
        }
    }

    #[test]
    fn action_default_is_ask() {
        assert_eq!(PermissionAction::default(), PermissionAction::Ask);
    }

    #[test]
    fn action_as_str_matches_serde() {
        for action in [
            PermissionAction::Allow,
            PermissionAction::Ask,
            PermissionAction::Deny,
        ] {
            assert_eq!(
                action.as_str(),
                serde_json::to_string(&action).unwrap().trim_matches('"')
            );
        }
    }

    #[test]
    fn rule_constructors_set_action() {
        assert_eq!(
            PermissionRule::allow("bash", "git *").action,
            PermissionAction::Allow
        );
        assert_eq!(
            PermissionRule::ask("bash", "rm *").action,
            PermissionAction::Ask
        );
        assert_eq!(
            PermissionRule::deny("edit", "*.env").action,
            PermissionAction::Deny
        );
    }
}
