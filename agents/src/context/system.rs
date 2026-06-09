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
