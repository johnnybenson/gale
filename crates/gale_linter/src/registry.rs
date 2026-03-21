use crate::rule::Rule;
use crate::rules;

/// A collection of registered lint rules.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a rule in the registry.
    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Look up a rule by name.
    pub fn get(&self, name: &str) -> Option<&dyn Rule> {
        self.rules.iter().find(|r| r.name() == name).map(|r| &**r)
    }

    /// Return a slice of all registered rules.
    pub fn all(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }
}

impl Default for RuleRegistry {
    /// Create a registry with all built-in rules registered.
    fn default() -> Self {
        let mut registry = Self::new();
        rules::register_all(&mut registry);
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_rules() {
        let registry = RuleRegistry::default();
        assert!(!registry.all().is_empty());
    }

    #[test]
    fn lookup_by_name() {
        let registry = RuleRegistry::default();
        assert!(registry.get("block-no-empty").is_some());
        assert!(registry.get("nonexistent-rule").is_none());
    }
}
