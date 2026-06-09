#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ContextBudget {
    pub max_input_tokens: usize,
    pub max_output_tokens: usize,
    pub compaction_threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BudgetDecision {
    Continue,
    Compact,
    Reject,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_input_tokens: 128_000,
            max_output_tokens: 8_000,
            compaction_threshold: 0.85,
        }
    }
}

impl ContextBudget {
    pub fn decide(&self, estimated_input_tokens: usize) -> BudgetDecision {
        if estimated_input_tokens + self.max_output_tokens > self.max_input_tokens {
            return BudgetDecision::Reject;
        }

        let threshold = (self.max_input_tokens as f32 * self.compaction_threshold) as usize;
        if estimated_input_tokens >= threshold {
            return BudgetDecision::Compact;
        }

        BudgetDecision::Continue
    }
}
