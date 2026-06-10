use std::collections::HashSet;
use std::sync::Arc;

use crate::context::{ContextSnapshot, ContextSource, LoadedContext};

#[derive(Default)]
pub struct SystemContext {
    sources: Vec<Arc<dyn ContextSource>>,
}

impl SystemContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source(mut self, source: Arc<dyn ContextSource>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn add_source(&mut self, source: Arc<dyn ContextSource>) {
        self.sources.push(source);
    }

    pub fn set_source(&mut self, key: &str, source: Arc<dyn ContextSource>) -> Result<(), String> {
        if source.key() != key {
            return Err(format!(
                "Source key mismatch: expected '{}', got '{}'",
                key,
                source.key()
            ));
        }
        for existing in self.sources.iter_mut() {
            if existing.key() == key {
                *existing = source;
                return Ok(());
            }
        }
        self.sources.push(source);
        Ok(())
    }

    pub fn remove_source(&mut self, key: &str) -> Option<Arc<dyn ContextSource>> {
        if let Some(index) = self.sources.iter().position(|source| source.key() == key) {
            Some(self.sources.remove(index))
        } else {
            None
        }
    }

    pub fn sources(&self) -> &[Arc<dyn ContextSource>] {
        &self.sources
    }

    pub fn initialize(&self) -> Result<(String, Vec<ContextSnapshot>), String> {
        let loaded = self.load_all()?;
        Ok((
            render(&loaded),
            loaded.iter().map(LoadedContext::snapshot).collect(),
        ))
    }

    pub fn reconcile(
        &self,
        previous: &[ContextSnapshot],
    ) -> Result<Option<(String, Vec<ContextSnapshot>)>, String> {
        let loaded = self.load_all()?;
        let snapshots: Vec<_> = loaded.iter().map(LoadedContext::snapshot).collect();
        if snapshots_equal(previous, &snapshots) {
            return Ok(None);
        }
        Ok(Some((render(&loaded), snapshots)))
    }

    fn load_all(&self) -> Result<Vec<LoadedContext>, String> {
        let mut keys = HashSet::new();
        let mut loaded = Vec::new();
        for source in &self.sources {
            if !keys.insert(source.key().to_string()) {
                return Err(format!("Duplicate context source key: {}", source.key()));
            }
            loaded.push(source.load()?);
        }
        Ok(loaded)
    }
}

fn render(parts: &[LoadedContext]) -> String {
    parts
        .iter()
        .map(|part| part.rendered.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn snapshots_equal(previous: &[ContextSnapshot], next: &[ContextSnapshot]) -> bool {
    previous.len() == next.len()
        && previous
            .iter()
            .zip(next.iter())
            .all(|(a, b)| a.key == b.key && a.value == b.value && a.rendered == b.rendered)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::*;
    use crate::context::budget::{BudgetDecision, ContextBudget};

    struct StaticSource {
        key: String,
        value: Value,
        rendered: String,
    }

    impl ContextSource for StaticSource {
        fn key(&self) -> &str {
            &self.key
        }
        fn load(&self) -> Result<LoadedContext, String> {
            Ok(LoadedContext {
                key: self.key.clone(),
                value: self.value.clone(),
                rendered: self.rendered.clone(),
            })
        }
    }

    fn estimate_tokens(text: &str) -> usize {
        text.split_whitespace().count().max(1)
    }

    fn log_prompt(label: &str, prompt: &str) {
        println!("\n========== {} ==========", label);
        println!("rendered prompt ({} chars, ~{} tokens):", prompt.len(), estimate_tokens(prompt));
        println!("----------------------------------------");
        println!("{}", prompt);
        println!("----------------------------------------");
    }

    fn log_snapshots(label: &str, snapshots: &[ContextSnapshot]) {
        println!("\n[snapshots: {}] ({} entries)", label, snapshots.len());
        for snap in snapshots {
            println!("  - key={}  value={}  rendered_len={}", snap.key, snap.value, snap.rendered.len());
        }
    }

    #[test]
    fn initialize_renders_aggregated_prompt() {
        let ctx = SystemContext::new()
            .with_source(Arc::new(StaticSource {
                key: "env".to_string(),
                value: json!({"os": "linux"}),
                rendered: "OS: linux".to_string(),
            }))
            .with_source(Arc::new(StaticSource {
                key: "project".to_string(),
                value: json!({"name": "openman"}),
                rendered: "Project: openman".to_string(),
            }));

        let (prompt, snapshots) = ctx.initialize().unwrap();

        log_prompt("initialize_renders_aggregated_prompt", &prompt);
        log_snapshots("initialize_renders_aggregated_prompt", &snapshots);

        assert_eq!(prompt, "OS: linux\n\nProject: openman");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].key, "env");
        assert_eq!(snapshots[1].key, "project");
    }

    #[test]
    fn empty_rendered_parts_are_skipped() {
        let ctx = SystemContext::new()
            .with_source(Arc::new(StaticSource {
                key: "a".to_string(),
                value: json!(null),
                rendered: "   ".to_string(),
            }))
            .with_source(Arc::new(StaticSource {
                key: "b".to_string(),
                value: json!(null),
                rendered: "".to_string(),
            }))
            .with_source(Arc::new(StaticSource {
                key: "c".to_string(),
                value: json!(null),
                rendered: "kept".to_string(),
            }));

        let (prompt, _) = ctx.initialize().unwrap();

        log_prompt("empty_rendered_parts_are_skipped", &prompt);
        println!("[a,b,c with empty/whitespace renderings -> only 'kept' should appear]");

        assert_eq!(prompt, "kept");
    }

    #[test]
    fn duplicate_source_keys_are_rejected() {
        let ctx = SystemContext::new()
            .with_source(Arc::new(StaticSource {
                key: "dup".to_string(),
                value: json!(1),
                rendered: "one".to_string(),
            }))
            .with_source(Arc::new(StaticSource {
                key: "dup".to_string(),
                value: json!(2),
                rendered: "two".to_string(),
            }));

        let err = ctx.initialize().unwrap_err();
        println!("\n[duplicate_source_keys_are_rejected] error: {}", err);
        assert!(err.contains("Duplicate context source key"));
        assert!(err.contains("dup"));
    }

    #[test]
    fn reconcile_returns_none_when_unchanged() {
        let ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "k".to_string(),
            value: json!("v"),
            rendered: "value".to_string(),
        }));

        let (initial_prompt, snapshots) = ctx.initialize().unwrap();
        let result = ctx.reconcile(&snapshots).unwrap();

        log_prompt("reconcile_returns_none_when_unchanged (initial)", &initial_prompt);
        log_snapshots("reconcile_returns_none_when_unchanged", &snapshots);
        println!("[reconcile result when unchanged] -> {:?} (expected None)", result);

        assert!(result.is_none());
    }

    #[test]
    fn reconcile_returns_new_prompt_when_changed() {
        let ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "k".to_string(),
            value: json!("v1"),
            rendered: "value-v1".to_string(),
        }));

        let (initial_prompt, snapshots) = ctx.initialize().unwrap();
        let mut ctx = ctx;
        ctx.set_source(
            "k",
            Arc::new(StaticSource {
                key: "k".to_string(),
                value: json!("v2"),
                rendered: "value-v2".to_string(),
            }),
        )
        .unwrap();

        let result = ctx.reconcile(&snapshots).unwrap();
        let (prompt, new_snapshots) = result.expect("expected change to be detected");

        log_prompt("reconcile_returns_new_prompt_when_changed (before)", &initial_prompt);
        log_prompt("reconcile_returns_new_prompt_when_changed (after)", &prompt);
        log_snapshots("reconcile_returns_new_prompt_when_changed (new)", &new_snapshots);

        assert_eq!(prompt, "value-v2");
        assert_eq!(new_snapshots.len(), 1);
        assert_eq!(new_snapshots[0].value, json!("v2"));
    }

    #[test]
    fn add_source_appends_new_entry() {
        let mut ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "a".to_string(),
            value: json!(1),
            rendered: "alpha".to_string(),
        }));

        ctx.add_source(Arc::new(StaticSource {
            key: "b".to_string(),
            value: json!(2),
            rendered: "beta".to_string(),
        }));

        assert_eq!(ctx.sources().len(), 2);
        let (prompt, _) = ctx.initialize().unwrap();

        log_prompt("add_source_appends_new_entry", &prompt);
        println!("[sources in order] -> {:?}", ctx.sources().iter().map(|s| s.key()).collect::<Vec<_>>());

        assert_eq!(prompt, "alpha\n\nbeta");
    }

    #[test]
    fn set_source_replaces_existing_entry() {
        let mut ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "k".to_string(),
            value: json!("old"),
            rendered: "old-render".to_string(),
        }));

        ctx.set_source(
            "k",
            Arc::new(StaticSource {
                key: "k".to_string(),
                value: json!("new"),
                rendered: "new-render".to_string(),
            }),
        )
        .unwrap();

        assert_eq!(ctx.sources().len(), 1);
        let (prompt, _) = ctx.initialize().unwrap();

        log_prompt("set_source_replaces_existing_entry", &prompt);
        println!("[replaced 'old-render' with 'new-render']");

        assert_eq!(prompt, "new-render");
    }

    #[test]
    fn set_source_rejects_key_mismatch() {
        let mut ctx = SystemContext::new();
        let err = ctx
            .set_source(
                "expected",
                Arc::new(StaticSource {
                    key: "actual".to_string(),
                    value: json!(null),
                    rendered: String::new(),
                }),
            )
            .unwrap_err();
        println!("\n[set_source_rejects_key_mismatch] error: {}", err);
        assert!(err.contains("key mismatch"));
    }

    #[test]
    fn remove_source_drops_entry() {
        let mut ctx = SystemContext::new()
            .with_source(Arc::new(StaticSource {
                key: "a".to_string(),
                value: json!(1),
                rendered: "alpha".to_string(),
            }))
            .with_source(Arc::new(StaticSource {
                key: "b".to_string(),
                value: json!(2),
                rendered: "beta".to_string(),
            }));

        let removed = ctx.remove_source("a").unwrap();
        assert_eq!(removed.key(), "a");
        assert_eq!(ctx.sources().len(), 1);

        let (prompt, _) = ctx.initialize().unwrap();

        log_prompt("remove_source_drops_entry", &prompt);
        println!("[removed source key='a'] remaining keys -> {:?}", ctx.sources().iter().map(|s| s.key()).collect::<Vec<_>>());

        assert_eq!(prompt, "beta");
    }

    #[test]
    fn remove_source_returns_none_when_missing() {
        let mut ctx = SystemContext::new();
        let result = ctx.remove_source("nope");
        println!("\n[remove_source_returns_none_when_missing] result: {:?} (expected None)", result.as_ref().map(|s| s.key().to_string()));
        assert!(result.is_none());
    }

    #[test]
    fn aggregated_prompt_within_budget_allows_continue() {
        let ctx = SystemContext::new()
            .with_source(Arc::new(StaticSource {
                key: "short".to_string(),
                value: json!({}),
                rendered: "just a few words".to_string(),
            }));

        let (prompt, _) = ctx.initialize().unwrap();
        let budget = ContextBudget::default();
        let tokens = estimate_tokens(&prompt);
        let decision = budget.decide(tokens);

        log_prompt("aggregated_prompt_within_budget_allows_continue", &prompt);
        println!("[tokens={}, budget max_input={}, max_output={}, threshold={}%]",
            tokens, budget.max_input_tokens, budget.max_output_tokens, (budget.compaction_threshold * 100.0) as usize);
        println!("[decision: {:?} (expected Continue)]", decision);

        assert_eq!(decision, BudgetDecision::Continue);
    }

    #[test]
    fn aggregated_prompt_over_compaction_threshold_triggers_compact() {
        let large = "word ".repeat(110_000);
        let ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "big".to_string(),
            value: json!({}),
            rendered: large.clone(),
        }));

        let (prompt, _) = ctx.initialize().unwrap();
        let budget = ContextBudget::default();
        let tokens = estimate_tokens(&prompt);
        let decision = budget.decide(tokens);

        println!("\n[aggregated_prompt_over_compaction_threshold_triggers_compact]");
        println!("[prompt length: {} chars, ~{} tokens (first 80 chars shown)]", prompt.len(), tokens);
        println!("[preview] {}", &prompt[..80.min(prompt.len())]);
        println!("[budget: max_input={}, max_output={}, threshold={}%]",
            budget.max_input_tokens, budget.max_output_tokens, (budget.compaction_threshold * 100.0) as usize);
        println!("[decision: {:?} (expected Compact)]", decision);

        assert_eq!(decision, BudgetDecision::Compact);
    }

    #[test]
    fn aggregated_prompt_plus_output_exceeding_budget_is_rejected() {
        let huge = "w ".repeat(150_000);
        let ctx = SystemContext::new().with_source(Arc::new(StaticSource {
            key: "huge".to_string(),
            value: json!({}),
            rendered: huge,
        }));

        let (prompt, _) = ctx.initialize().unwrap();
        let budget = ContextBudget::default();
        let tokens = estimate_tokens(&prompt);
        let decision = budget.decide(tokens);

        println!("\n[aggregated_prompt_plus_output_exceeding_budget_is_rejected]");
        println!("[prompt length: {} chars, ~{} tokens (first 80 chars shown)]", prompt.len(), tokens);
        println!("[preview] {}", &prompt[..80.min(prompt.len())]);
        println!("[budget: max_input={}, max_output={}, threshold={}%]",
            budget.max_input_tokens, budget.max_output_tokens, (budget.compaction_threshold * 100.0) as usize);
        println!("[decision: {:?} (expected Reject)]", decision);

        assert_eq!(decision, BudgetDecision::Reject);
    }
}
