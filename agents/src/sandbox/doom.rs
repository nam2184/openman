use std::collections::VecDeque;
use std::sync::Mutex;

/// Tracks the last N tool calls and flags a doom loop when the same call
/// repeats at least `threshold` times. Per-session state.
pub struct DoomLoopDetector {
    threshold: usize,
    history: Mutex<VecDeque<ToolCallFingerprint>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallFingerprint {
    pub tool: String,
    /// Serialized arguments (we don't deeply normalize, just join).
    pub args: String,
}

impl DoomLoopDetector {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            history: Mutex::new(VecDeque::with_capacity(threshold * 2)),
        }
    }

    /// Record a tool call and return whether it triggers a doom loop.
    /// Returns `true` if the most recent `threshold` calls are all the same.
    pub fn record(&self, tool: &str, args: &str) -> bool {
        let fingerprint = ToolCallFingerprint {
            tool: tool.to_string(),
            args: args.to_string(),
        };
        let mut history = self.history.lock().expect("doom lock poisoned");
        history.push_back(fingerprint);
        while history.len() > self.threshold {
            history.pop_front();
        }
        if history.len() < self.threshold {
            return false;
        }
        // All entries in the window must match the most recent one.
        let last = history.back().unwrap();
        history.iter().all(|fp| fp == last)
    }

    /// Reset the history (e.g., after the user explicitly approves a repeat).
    pub fn reset(&self) {
        self.history.lock().expect("doom lock poisoned").clear();
    }

    pub fn history_len(&self) -> usize {
        self.history.lock().expect("doom lock poisoned").len()
    }
}

impl Default for DoomLoopDetector {
    fn default() -> Self {
        // opencode uses 3.
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_call_does_not_trigger() {
        let detector = DoomLoopDetector::new(3);
        assert!(!detector.record("bash", "ls"));
    }

    #[test]
    fn two_repeats_does_not_trigger() {
        let detector = DoomLoopDetector::new(3);
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("bash", "ls"));
    }

    #[test]
    fn three_repeats_triggers() {
        let detector = DoomLoopDetector::new(3);
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("bash", "ls"));
        assert!(detector.record("bash", "ls"));
    }

    #[test]
    fn different_args_dont_trigger() {
        let detector = DoomLoopDetector::new(3);
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("bash", "pwd"));
        assert!(!detector.record("bash", "echo"));
    }

    #[test]
    fn different_tools_dont_trigger() {
        let detector = DoomLoopDetector::new(3);
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("read", "ls"));
        assert!(!detector.record("grep", "ls"));
    }

    #[test]
    fn reset_clears_history() {
        let detector = DoomLoopDetector::new(3);
        detector.record("bash", "ls");
        detector.record("bash", "ls");
        detector.reset();
        assert_eq!(detector.history_len(), 0);
        // After reset, a single call doesn't trigger.
        assert!(!detector.record("bash", "ls"));
    }

    #[test]
    fn history_keeps_only_threshold_size() {
        let detector = DoomLoopDetector::new(3);
        // 10 different calls; only the last 3 are kept.
        for i in 0..10 {
            detector.record("bash", &format!("call-{i}"));
        }
        assert_eq!(detector.history_len(), 3);
    }

    #[test]
    fn different_in_window_does_not_trigger() {
        let detector = DoomLoopDetector::new(3);
        detector.record("bash", "ls");
        detector.record("bash", "ls");
        detector.record("bash", "pwd"); // breaks the streak
        // Now two more "ls" should bring it back to 2/3, not 3/3.
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("bash", "ls"));
    }

    #[test]
    fn doom_loop_triggers_again_after_window_overflow() {
        let detector = DoomLoopDetector::new(3);
        detector.record("bash", "ls");
        detector.record("bash", "ls");
        assert!(detector.record("bash", "ls")); // 3rd triggers
        // The 3rd call bumped it; 4th is still all-same.
        assert!(detector.record("bash", "ls"));
    }

    #[test]
    fn default_threshold_is_three() {
        let detector = DoomLoopDetector::default();
        assert!(!detector.record("bash", "ls"));
        assert!(!detector.record("bash", "ls"));
        assert!(detector.record("bash", "ls"));
    }
}
