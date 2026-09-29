//! Variable scopes: one per file, plus a process wide global scope.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::logger;

/// A value of a variable. [`None`] marks an explicitly unset variable, which
/// shadows a global variable of the same name.
pub type Value = Option<String>;

/// The process wide scope, seeded from the environment on first use.
fn global() -> &'static Mutex<HashMap<String, Value>> {
    static GLOBAL: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    GLOBAL.get_or_init(|| {
        let mut scope = HashMap::new();
        for (key, value) in std::env::vars() {
            scope.insert(key, Some(value));
        }
        Mutex::new(scope)
    })
}

/// A set of variables with a lookup that falls back to the global scope.
#[derive(Debug, Default, Clone)]
pub struct VariableScope {
    name: String,
    local: HashMap<String, Value>,
}

impl VariableScope {
    /// Creates an empty scope named `name`, used in log messages.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            local: HashMap::new(),
        }
    }

    /// The name of the scope.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets a variable in this scope only.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.local.insert(key.into(), Some(value.into()));
    }

    /// Unsets a variable in this scope only.
    pub fn unset(&mut self, key: impl Into<String>) {
        self.local.insert(key.into(), None);
    }

    /// Copies every variable of `other` into this scope.
    pub fn extend(&mut self, other: &VariableScope) -> &mut Self {
        for (key, value) in &other.local {
            self.local.insert(key.clone(), value.clone());
        }
        self
    }

    /// Copies every entry of `other` into this scope.
    pub fn extend_from<I, K, V>(&mut self, other: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in other {
            self.local.insert(key.into(), Some(value.into()));
        }
        self
    }

    /// Reads a variable, falling back to the global scope.
    pub fn get(&self, key: &str) -> Option<String> {
        match self.local.get(key) {
            Some(value) => value.clone(),
            None => Self::global_get(key),
        }
    }

    /// Reports whether the local scope knows about `key`.
    pub fn contains(&self, key: &str) -> bool {
        self.local.contains_key(key)
    }

    /// Reads a variable of the global scope.
    pub fn global_get(key: &str) -> Option<String> {
        global()
            .lock()
            .ok()
            .and_then(|scope| scope.get(key).cloned().flatten())
    }

    /// Writes a variable into the global scope.
    pub fn global_set(key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = Some(value.into());
        if let Ok(mut scope) = global().lock() {
            scope.insert(key.clone(), value);
        }
        logger::log(
            logger::Level::Debug,
            "scope",
            &format!("global {key} is set"),
        );
    }

    /// Unsets a variable of the global scope.
    pub fn global_unset(key: impl Into<String>) {
        let key = key.into();
        if let Ok(mut scope) = global().lock() {
            scope.insert(key.clone(), None);
        }
        logger::log(
            logger::Level::Debug,
            "scope",
            &format!("global {key} is unset"),
        );
    }

    /// Removes every variable of the global scope, including the environment.
    pub fn clear_globals() {
        if let Ok(mut scope) = global().lock() {
            scope.clear();
        }
    }

    /// Every variable of the global scope, sorted by name.
    pub fn global_variables() -> Vec<(String, String)> {
        let Ok(scope) = global().lock() else {
            return Vec::new();
        };
        let mut variables: Vec<(String, String)> = scope
            .iter()
            .filter_map(|(key, value)| value.clone().map(|value| (key.clone(), value)))
            .collect();
        variables.sort();
        variables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_and_reads_a_variable() {
        let mut scope = VariableScope::new("test");
        scope.set("key", "value");
        assert_eq!(scope.get("key").as_deref(), Some("value"));
    }

    #[test]
    fn unsets_a_variable() {
        let mut scope = VariableScope::new("test");
        scope.set("key", "value");
        scope.unset("key");
        assert_eq!(scope.get("key"), None);
    }

    #[test]
    fn reports_the_name() {
        assert_eq!(VariableScope::new("builder").name(), "builder");
    }

    #[test]
    fn extends_another_scope() {
        let mut parent = VariableScope::new("parent");
        parent.set("key", "value");
        let mut child = VariableScope::new("child");
        child.extend(&parent);
        assert_eq!(child.get("key").as_deref(), Some("value"));
    }

    #[test]
    fn extends_from_pairs() {
        let mut scope = VariableScope::new("scope");
        scope.extend_from([("a", "1"), ("b", "2")]);
        assert_eq!(scope.get("a").as_deref(), Some("1"));
        assert_eq!(scope.get("b").as_deref(), Some("2"));
    }

    #[test]
    fn knows_about_local_variables() {
        let mut scope = VariableScope::new("scope");
        assert!(!scope.contains("key"));
        scope.set("key", "value");
        assert!(scope.contains("key"));
    }

    #[test]
    fn the_global_scope_is_shared() {
        VariableScope::global_set("rpsql_test_key", "value");
        let scope = VariableScope::new("scope");
        assert_eq!(scope.get("rpsql_test_key").as_deref(), Some("value"));
        assert_eq!(
            VariableScope::global_get("rpsql_test_key").as_deref(),
            Some("value")
        );
        VariableScope::global_unset("rpsql_test_key");
        assert_eq!(VariableScope::global_get("rpsql_test_key"), None);
    }

    #[test]
    fn the_local_scope_shadows_the_global_scope() {
        VariableScope::global_set("rpsql_test_shadow", "global");
        let mut scope = VariableScope::new("scope");
        scope.set("rpsql_test_shadow", "local");
        assert_eq!(scope.get("rpsql_test_shadow").as_deref(), Some("local"));
        assert_eq!(
            VariableScope::global_get("rpsql_test_shadow").as_deref(),
            Some("global")
        );
    }

    #[test]
    fn a_local_unset_shadows_a_global_variable() {
        VariableScope::global_set("rpsql_test_hidden", "global");
        let mut scope = VariableScope::new("scope");
        scope.unset("rpsql_test_hidden");
        assert_eq!(scope.get("rpsql_test_hidden"), None);
    }

    #[test]
    fn lists_the_global_variables_sorted() {
        VariableScope::global_set("rpsql_sorted_b", "2");
        VariableScope::global_set("rpsql_sorted_a", "1");
        let variables = VariableScope::global_variables();
        let found: Vec<&str> = variables
            .iter()
            .filter(|(key, _)| key.starts_with("rpsql_sorted_"))
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(found, ["rpsql_sorted_a", "rpsql_sorted_b"]);
    }
}
