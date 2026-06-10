#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BudgetOverride {
    pub max_input_tokens: Option<usize>,
    pub max_output_tokens: Option<usize>,
    pub compaction_threshold: Option<f32>,
}

impl Default for BudgetOverride {
    fn default() -> Self {
        Self {
            max_input_tokens: None,
            max_output_tokens: None,
            compaction_threshold: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClampedBudget {
    pub budget: ContextBudget,
    pub clamped: bool,
}

impl ContextBudget {
    pub fn for_model(context_window: usize, max_output: usize) -> Self {
        Self {
            max_input_tokens: context_window,
            max_output_tokens: max_output,
            compaction_threshold: 0.85,
        }
    }

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

    pub fn apply_override(&self, ceiling: &Self, override_settings: &BudgetOverride) -> ClampedBudget {
        let mut clamped = false;
        // Start from the model's ceiling as the base. Self is the model budget.
        let mut next = *ceiling;

        if let Some(input) = override_settings.max_input_tokens {
            if input > ceiling.max_input_tokens {
                clamped = true;
                // next.max_input_tokens stays at ceiling
            } else {
                next.max_input_tokens = input;
            }
        }
        if let Some(output) = override_settings.max_output_tokens {
            if output > ceiling.max_output_tokens {
                clamped = true;
            } else {
                next.max_output_tokens = output;
            }
        }
        if let Some(threshold) = override_settings.compaction_threshold {
            if !(0.0..=1.0).contains(&threshold) {
                clamped = true;
            } else {
                next.compaction_threshold = threshold;
            }
        }

        ClampedBudget {
            budget: next,
            clamped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_decision(label: &str, budget: &ContextBudget, input: usize) -> BudgetDecision {
        let decision = budget.decide(input);
        let threshold = (budget.max_input_tokens as f32 * budget.compaction_threshold) as usize;
        println!(
            "\n[{}] input={} max_input={} max_output={} threshold={} -> {:?}",
            label, input, budget.max_input_tokens, budget.max_output_tokens, threshold, decision
        );
        decision
    }

    #[test]
    fn default_budget_values() {
        let budget = ContextBudget::default();
        println!(
            "\n[default_budget_values] max_input={} max_output={} compaction_threshold={}",
            budget.max_input_tokens, budget.max_output_tokens, budget.compaction_threshold
        );
        assert_eq!(budget.max_input_tokens, 128_000);
        assert_eq!(budget.max_output_tokens, 8_000);
        assert!((budget.compaction_threshold - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn small_input_returns_continue() {
        let budget = ContextBudget::default();
        let decision = log_decision("small_input_returns_continue", &budget, 1_000);
        assert_eq!(decision, BudgetDecision::Continue);
    }

    #[test]
    fn input_past_compaction_threshold_returns_compact() {
        let budget = ContextBudget {
            max_input_tokens: 1_000,
            max_output_tokens: 100,
            compaction_threshold: 0.8,
        };
        // threshold = 800
        let d1 = log_decision("input_past_compaction_threshold_returns_compact (800)", &budget, 800);
        let d2 = log_decision("input_past_compaction_threshold_returns_compact (900)", &budget, 900);
        assert_eq!(d1, BudgetDecision::Compact);
        assert_eq!(d2, BudgetDecision::Compact);
    }

    #[test]
    fn input_plus_output_over_limit_returns_reject() {
        let budget = ContextBudget {
            max_input_tokens: 1_000,
            max_output_tokens: 200,
            compaction_threshold: 0.8,
        };
        // 900 + 200 > 1000 -> Reject
        let decision = log_decision("input_plus_output_over_limit_returns_reject (900)", &budget, 900);
        assert_eq!(decision, BudgetDecision::Reject);
    }

    #[test]
    fn input_at_exact_limit_still_rejected() {
        let budget = ContextBudget {
            max_input_tokens: 1_000,
            max_output_tokens: 200,
            compaction_threshold: 0.8,
        };
        // 1000 + 200 > 1000 -> Reject
        let decision = log_decision("input_at_exact_limit_still_rejected (1000)", &budget, 1_000);
        assert_eq!(decision, BudgetDecision::Reject);
    }

    #[test]
    fn input_below_threshold_returns_continue() {
        let budget = ContextBudget {
            max_input_tokens: 1_000,
            max_output_tokens: 100,
            compaction_threshold: 0.5,
        };
        // threshold = 500, so 499 should Continue
        let decision = log_decision("input_below_threshold_returns_continue (499)", &budget, 499);
        assert_eq!(decision, BudgetDecision::Continue);
    }

    #[test]
    fn for_model_uses_provided_window_and_output() {
        let budget = ContextBudget::for_model(1_048_576, 32_768);
        println!(
            "\n[for_model_uses_provided_window_and_output] context_window=1048576 max_output=32768 -> max_input={} max_output={}",
            budget.max_input_tokens, budget.max_output_tokens
        );
        assert_eq!(budget.max_input_tokens, 1_048_576);
        assert_eq!(budget.max_output_tokens, 32_768);
        assert!((budget.compaction_threshold - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn override_within_ceiling_is_applied() {
        let ceiling = ContextBudget::for_model(200_000, 64_000);
        let override_settings = BudgetOverride {
            max_input_tokens: Some(150_000),
            max_output_tokens: Some(32_000),
            compaction_threshold: Some(0.75),
        };
        let result = ceiling.apply_override(&ceiling, &override_settings);
        println!(
            "\n[override_within_ceiling_is_applied] ceiling=(200000,64000) override=(150000,32000,0.75) -> budget=({},{},{}) clamped={}",
            result.budget.max_input_tokens,
            result.budget.max_output_tokens,
            result.budget.compaction_threshold,
            result.clamped
        );
        assert!(!result.clamped);
        assert_eq!(result.budget.max_input_tokens, 150_000);
        assert_eq!(result.budget.max_output_tokens, 32_000);
        assert!((result.budget.compaction_threshold - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn override_exceeding_ceiling_is_clamped() {
        let ceiling = ContextBudget::for_model(200_000, 64_000);
        let override_settings = BudgetOverride {
            max_input_tokens: Some(500_000),
            max_output_tokens: Some(100_000),
            compaction_threshold: Some(0.9),
        };
        let result = ceiling.apply_override(&ceiling, &override_settings);
        println!(
            "\n[override_exceeding_ceiling_is_clamped] ceiling=(200000,64000) override=(500000,100000,0.9) -> budget=({},{},{}) clamped={}",
            result.budget.max_input_tokens,
            result.budget.max_output_tokens,
            result.budget.compaction_threshold,
            result.clamped
        );
        assert!(result.clamped);
        // Clamped values stay at the ceiling
        assert_eq!(result.budget.max_input_tokens, 200_000);
        assert_eq!(result.budget.max_output_tokens, 64_000);
    }

    #[test]
    fn override_can_lower_without_clamping() {
        let ceiling = ContextBudget::for_model(200_000, 64_000);
        let override_settings = BudgetOverride {
            max_input_tokens: Some(50_000),
            max_output_tokens: None,
            compaction_threshold: None,
        };
        let result = ceiling.apply_override(&ceiling, &override_settings);
        println!(
            "\n[override_can_lower_without_clamping] ceiling=(200000,64000) override input=50000 -> budget=({},{}) clamped={}",
            result.budget.max_input_tokens, result.budget.max_output_tokens, result.clamped
        );
        assert!(!result.clamped);
        assert_eq!(result.budget.max_input_tokens, 50_000);
        // Output not overridden, stays at ceiling default
        assert_eq!(result.budget.max_output_tokens, 64_000);
    }

    #[test]
    fn override_invalid_threshold_is_rejected() {
        let ceiling = ContextBudget::for_model(200_000, 64_000);
        let override_settings = BudgetOverride {
            max_input_tokens: None,
            max_output_tokens: None,
            compaction_threshold: Some(1.5),
        };
        let result = ceiling.apply_override(&ceiling, &override_settings);
        println!(
            "\n[override_invalid_threshold_is_rejected] threshold=1.5 -> budget threshold={} clamped={}",
            result.budget.compaction_threshold, result.clamped
        );
        assert!(result.clamped);
        // Stays at default since invalid value rejected
        assert!((result.budget.compaction_threshold - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn no_override_returns_model_default() {
        let ceiling = ContextBudget::for_model(1_048_576, 32_768);
        let result = ceiling.apply_override(&ceiling, &BudgetOverride::default());
        println!(
            "\n[no_override_returns_model_default] ceiling=(1048576,32768) -> budget=({},{}) clamped={}",
            result.budget.max_input_tokens, result.budget.max_output_tokens, result.clamped
        );
        assert!(!result.clamped);
        assert_eq!(result.budget.max_input_tokens, 1_048_576);
        assert_eq!(result.budget.max_output_tokens, 32_768);
    }

    #[test]
    fn override_ceiling_distinct_from_base() {
        // Base budget is 200k, but ceiling caps to 100k for some policy reason
        let base = ContextBudget::for_model(200_000, 64_000);
        let ceiling = ContextBudget::for_model(100_000, 32_000);
        let override_settings = BudgetOverride {
            max_input_tokens: Some(150_000), // exceeds ceiling
            max_output_tokens: Some(20_000),
            compaction_threshold: None,
        };
        let result = base.apply_override(&ceiling, &override_settings);
        println!(
            "\n[override_ceiling_distinct_from_base] base=(200000,64000) ceiling=(100000,32000) override=(150000,20000) -> budget=({},{}) clamped={}",
            result.budget.max_input_tokens, result.budget.max_output_tokens, result.clamped
        );
        assert!(result.clamped);
        assert_eq!(result.budget.max_input_tokens, 100_000);
        assert_eq!(result.budget.max_output_tokens, 20_000); // within ceiling
    }
}
