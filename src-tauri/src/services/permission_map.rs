use arachne_agents::permission_v2::{default_ruleset, PermissionService};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Per-session permission service registry. Each session gets its own
/// `PermissionService` so that approved rules, pending requests, and
/// doom-loop state are scoped to that session.
pub struct PermissionMap {
    services: Mutex<HashMap<String, Arc<PermissionService>>>,
}

impl PermissionMap {
    pub fn new() -> Self {
        Self {
            services: Mutex::new(HashMap::new()),
        }
    }

    /// Get the service for `session_id`, creating one with the default
    /// ruleset if it doesn't exist.
    pub fn get_or_create(&self, session_id: &str) -> Arc<PermissionService> {
        let mut services = self.services.lock().unwrap();
        services
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let ruleset = default_ruleset();
                let (service, _rx) = PermissionService::new(session_id, ruleset);
                service
            })
            .clone()
    }

    /// Get the service for `session_id` if it exists, without creating.
    pub fn get(&self, session_id: &str) -> Option<Arc<PermissionService>> {
        self.services.lock().unwrap().get(session_id).cloned()
    }

    /// Remove the service for `session_id`.
    pub fn remove(&self, session_id: &str) -> Option<Arc<PermissionService>> {
        self.services.lock().unwrap().remove(session_id)
    }
}
