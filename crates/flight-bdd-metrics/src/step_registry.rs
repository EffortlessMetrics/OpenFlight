// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step registry mapping Gherkin step patterns to handler functions.
//!
//! Each step pattern is a regex. When a scenario step matches, the handler is
//! invoked with a shared [`StepContext`] and the regex [`Captures`].

use regex::{Captures, Regex};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared mutable context passed between steps within a single scenario.
///
/// Steps store and retrieve typed values by string key so that *Given* steps
/// can set up state consumed by *When* and *Then* steps.
///
/// Values are stored behind `Arc` so they can be shared without requiring `Clone`.
#[derive(Default)]
pub struct StepContext {
    data: Mutex<HashMap<String, Arc<dyn Any + Send + Sync>>>,
}

impl StepContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a typed value under `key`.
    pub fn set<T: Send + Sync + 'static>(&self, key: &str, value: T) {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), Arc::new(value));
    }

    /// Retrieve a shared reference to the value for `key`.
    ///
    /// Returns `None` if the key is missing or the type does not match.
    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.data
            .lock()
            .unwrap()
            .get(key)
            .and_then(|v| Arc::clone(v).downcast::<T>().ok())
    }

    /// Retrieve a cloned value for `key`. Requires `T: Clone`.
    pub fn get_cloned<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<T> {
        self.get::<T>(key).map(|arc| (*arc).clone())
    }

    /// Check whether `key` exists in the context.
    pub fn contains(&self, key: &str) -> bool {
        self.data.lock().unwrap().contains_key(key)
    }

    /// Remove all stored data (called between scenarios).
    pub fn clear(&self) {
        self.data.lock().unwrap().clear();
    }
}

/// A step handler function signature.
pub type StepHandler = Arc<dyn Fn(&StepContext, &Captures<'_>) -> StepOutcome + Send + Sync>;

/// Outcome of executing a single step.
#[derive(Debug, Clone)]
pub enum StepOutcome {
    Passed,
    Failed(String),
    Skipped(String),
}

impl StepOutcome {
    pub fn is_passed(&self) -> bool {
        matches!(self, StepOutcome::Passed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, StepOutcome::Failed(_))
    }
}

/// A compiled step entry: regex pattern + handler.
struct StepEntry {
    pattern: Regex,
    handler: StepHandler,
}

/// Registry of Given/When/Then step definitions.
pub struct StepRegistry {
    given: Vec<StepEntry>,
    when: Vec<StepEntry>,
    then: Vec<StepEntry>,
}

impl StepRegistry {
    pub fn new() -> Self {
        Self {
            given: Vec::new(),
            when: Vec::new(),
            then: Vec::new(),
        }
    }

    /// Register a *Given* step pattern.
    pub fn given(
        &mut self,
        pattern: &str,
        handler: impl Fn(&StepContext, &Captures<'_>) -> StepOutcome + Send + Sync + 'static,
    ) {
        self.given.push(StepEntry {
            pattern: Regex::new(pattern).expect("invalid step regex"),
            handler: Arc::new(handler),
        });
    }

    /// Register a *When* step pattern.
    pub fn when(
        &mut self,
        pattern: &str,
        handler: impl Fn(&StepContext, &Captures<'_>) -> StepOutcome + Send + Sync + 'static,
    ) {
        self.when.push(StepEntry {
            pattern: Regex::new(pattern).expect("invalid step regex"),
            handler: Arc::new(handler),
        });
    }

    /// Register a *Then* step pattern.
    pub fn then(
        &mut self,
        pattern: &str,
        handler: impl Fn(&StepContext, &Captures<'_>) -> StepOutcome + Send + Sync + 'static,
    ) {
        self.then.push(StepEntry {
            pattern: Regex::new(pattern).expect("invalid step regex"),
            handler: Arc::new(handler),
        });
    }

    /// Try to find a matching *Given* handler for the step text.
    pub fn match_given<'t>(&self, text: &'t str) -> Option<(&StepHandler, Captures<'t>)> {
        Self::find_match(&self.given, text)
    }

    /// Try to find a matching *When* handler for the step text.
    pub fn match_when<'t>(&self, text: &'t str) -> Option<(&StepHandler, Captures<'t>)> {
        Self::find_match(&self.when, text)
    }

    /// Try to find a matching *Then* handler for the step text.
    pub fn match_then<'t>(&self, text: &'t str) -> Option<(&StepHandler, Captures<'t>)> {
        Self::find_match(&self.then, text)
    }

    /// Number of registered Given steps.
    pub fn given_count(&self) -> usize {
        self.given.len()
    }

    /// Number of registered When steps.
    pub fn when_count(&self) -> usize {
        self.when.len()
    }

    /// Number of registered Then steps.
    pub fn then_count(&self) -> usize {
        self.then.len()
    }

    fn find_match<'e, 't>(
        entries: &'e [StepEntry],
        text: &'t str,
    ) -> Option<(&'e StepHandler, Captures<'t>)> {
        for entry in entries {
            if let Some(caps) = entry.pattern.captures(text) {
                return Some((&entry.handler, caps));
            }
        }
        None
    }
}

impl Default for StepRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_set_get_roundtrip() {
        let ctx = StepContext::new();
        ctx.set("greeting", "hello".to_string());
        assert_eq!(
            ctx.get_cloned::<String>("greeting"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn context_returns_none_for_missing_key() {
        let ctx = StepContext::new();
        assert_eq!(ctx.get_cloned::<f32>("missing"), None);
    }

    #[test]
    fn context_returns_none_for_wrong_type() {
        let ctx = StepContext::new();
        ctx.set("num", 42_i32);
        assert_eq!(ctx.get_cloned::<String>("num"), None);
    }

    #[test]
    fn context_clear_removes_all() {
        let ctx = StepContext::new();
        ctx.set("a", 1_i32);
        ctx.set("b", 2_i32);
        ctx.clear();
        assert!(!ctx.contains("a"));
        assert!(!ctx.contains("b"));
    }

    #[test]
    fn registry_matches_given_pattern() {
        let mut reg = StepRegistry::new();
        reg.given(r"^a deadzone of (\d+\.\d+)$", |_ctx, _caps| {
            StepOutcome::Passed
        });
        assert!(reg.match_given("a deadzone of 0.05").is_some());
        assert!(reg.match_given("something else").is_none());
    }

    #[test]
    fn registry_captures_groups() {
        let mut reg = StepRegistry::new();
        reg.when(r"^input (-?\d+\.\d+) is processed$", |_ctx, caps| {
            let val: f32 = caps[1].parse().unwrap();
            assert!((val - 0.5).abs() < 1e-6);
            StepOutcome::Passed
        });
        let (handler, caps) = reg.match_when("input 0.5 is processed").unwrap();
        let outcome = handler(&StepContext::new(), &caps);
        assert!(outcome.is_passed());
    }

    #[test]
    fn registry_counts() {
        let mut reg = StepRegistry::new();
        reg.given("^a$", |_, _| StepOutcome::Passed);
        reg.given("^b$", |_, _| StepOutcome::Passed);
        reg.when("^c$", |_, _| StepOutcome::Passed);
        reg.then("^d$", |_, _| StepOutcome::Passed);
        reg.then("^e$", |_, _| StepOutcome::Passed);
        reg.then("^f$", |_, _| StepOutcome::Passed);
        assert_eq!(reg.given_count(), 2);
        assert_eq!(reg.when_count(), 1);
        assert_eq!(reg.then_count(), 3);
    }
}
