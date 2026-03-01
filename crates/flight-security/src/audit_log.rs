// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Audit logging for security-relevant events (REQ-924).
//!
//! Provides a fixed-capacity, in-process audit log that records security events
//! such as authentication, authorization decisions, plugin lifecycle, and
//! configuration changes. Entries rotate when the log reaches its configured
//! maximum size.

use std::collections::VecDeque;
use std::time::SystemTime;

/// Security-relevant event categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditCategory {
    Authentication,
    Authorization,
    ConfigChange,
    PluginLoad,
    PluginUnload,
    DeviceAccess,
    NetworkAccess,
    FileAccess,
    ServiceLifecycle,
    UpdateInstall,
}

/// Severity of the audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditSeverity {
    Info,
    Warning,
    Alert,
    Critical,
}

/// Outcome of the audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
}

/// A single audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: SystemTime,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
    pub actor: String,
    pub action: String,
    pub resource: String,
    pub outcome: AuditOutcome,
    pub details: Option<String>,
}

/// Audit log with configurable retention.
pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    max_entries: usize,
    enabled: bool,
}

impl AuditLog {
    /// Create a new audit log that retains at most `max_entries` entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries.min(1024)),
            max_entries,
            enabled: true,
        }
    }

    /// Record a pre-built entry. Oldest entries are dropped when the log is full.
    pub fn record(&mut self, entry: AuditEntry) {
        if !self.enabled {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Convenience method to record an event from individual fields.
    #[allow(clippy::too_many_arguments)]
    pub fn record_event(
        &mut self,
        category: AuditCategory,
        severity: AuditSeverity,
        actor: impl Into<String>,
        action: impl Into<String>,
        resource: impl Into<String>,
        outcome: AuditOutcome,
        details: Option<String>,
    ) {
        self.record(AuditEntry {
            timestamp: SystemTime::now(),
            category,
            severity,
            actor: actor.into(),
            action: action.into(),
            resource: resource.into(),
            outcome,
            details,
        });
    }

    /// All entries in insertion order.
    pub fn entries(&self) -> &VecDeque<AuditEntry> {
        &self.entries
    }

    /// Entries matching the given category.
    pub fn entries_by_category(&self, category: AuditCategory) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Entries at or above the given minimum severity.
    pub fn entries_by_severity(&self, min_severity: AuditSeverity) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.severity >= min_severity)
            .collect()
    }

    /// Entries performed by a specific actor.
    pub fn entries_by_actor(&self, actor: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.actor == actor).collect()
    }

    /// All entries with a non-success outcome (Failure or Denied).
    pub fn failures(&self) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.outcome != AuditOutcome::Success)
            .collect()
    }

    /// The most recent `count` entries (newest last).
    pub fn recent(&self, count: usize) -> Vec<&AuditEntry> {
        let len = self.entries.len();
        let skip = len.saturating_sub(count);
        self.entries.iter().skip(skip).collect()
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the audit log contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return entries matching a composite [`AuditFilter`].
    pub fn query(&self, filter: &AuditFilter) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| filter.matches(e)).collect()
    }

    /// Serialize all entries to a JSON array string.
    pub fn export_json(&self) -> String {
        use std::fmt::Write;

        let mut buf = String::from("[");
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            let ts = entry
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let details_json = match &entry.details {
                Some(d) => {
                    let escaped = d.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("\"{escaped}\"")
                }
                None => "null".to_string(),
            };
            let _ = write!(
                buf,
                concat!(
                    "{{",
                    "\"timestamp\":{ts},",
                    "\"category\":\"{cat}\",",
                    "\"severity\":\"{sev}\",",
                    "\"actor\":\"{actor}\",",
                    "\"action\":\"{action}\",",
                    "\"resource\":\"{resource}\",",
                    "\"outcome\":\"{outcome}\",",
                    "\"details\":{details}",
                    "}}"
                ),
                ts = ts,
                cat = fmt_category(entry.category),
                sev = fmt_severity(entry.severity),
                actor = entry.actor,
                action = entry.action,
                resource = entry.resource,
                outcome = fmt_outcome(entry.outcome),
                details = details_json,
            );
        }
        buf.push(']');
        buf
    }
}

/// Composite filter for querying the audit log.
///
/// All specified criteria must match (logical AND). An `Option::None` field
/// means "accept any value" for that criterion.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Minimum severity (inclusive).
    pub min_severity: Option<AuditSeverity>,
    /// Filter by category.
    pub category: Option<AuditCategory>,
    /// Filter by actor (source).
    pub actor: Option<String>,
    /// Only entries at or after this timestamp.
    pub from: Option<SystemTime>,
    /// Only entries at or before this timestamp.
    pub until: Option<SystemTime>,
}

impl AuditFilter {
    /// Build a new empty filter (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_severity(mut self, severity: AuditSeverity) -> Self {
        self.min_severity = Some(severity);
        self
    }

    pub fn with_category(mut self, category: AuditCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn with_time_range(mut self, from: SystemTime, until: SystemTime) -> Self {
        self.from = Some(from);
        self.until = Some(until);
        self
    }

    /// Check whether a single entry matches this filter.
    pub fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(min) = self.min_severity
            && entry.severity < min
        {
            return false;
        }
        if let Some(cat) = self.category
            && entry.category != cat
        {
            return false;
        }
        if let Some(ref actor) = self.actor
            && entry.actor != *actor
        {
            return false;
        }
        if let Some(from) = self.from
            && entry.timestamp < from
        {
            return false;
        }
        if let Some(until) = self.until
            && entry.timestamp > until
        {
            return false;
        }
        true
    }
}

fn fmt_category(c: AuditCategory) -> &'static str {
    match c {
        AuditCategory::Authentication => "Authentication",
        AuditCategory::Authorization => "Authorization",
        AuditCategory::ConfigChange => "ConfigChange",
        AuditCategory::PluginLoad => "PluginLoad",
        AuditCategory::PluginUnload => "PluginUnload",
        AuditCategory::DeviceAccess => "DeviceAccess",
        AuditCategory::NetworkAccess => "NetworkAccess",
        AuditCategory::FileAccess => "FileAccess",
        AuditCategory::ServiceLifecycle => "ServiceLifecycle",
        AuditCategory::UpdateInstall => "UpdateInstall",
    }
}

fn fmt_severity(s: AuditSeverity) -> &'static str {
    match s {
        AuditSeverity::Info => "Info",
        AuditSeverity::Warning => "Warning",
        AuditSeverity::Alert => "Alert",
        AuditSeverity::Critical => "Critical",
    }
}

fn fmt_outcome(o: AuditOutcome) -> &'static str {
    match o {
        AuditOutcome::Success => "Success",
        AuditOutcome::Failure => "Failure",
        AuditOutcome::Denied => "Denied",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        category: AuditCategory,
        severity: AuditSeverity,
        actor: &str,
        outcome: AuditOutcome,
    ) -> AuditEntry {
        AuditEntry {
            timestamp: SystemTime::now(),
            category,
            severity,
            actor: actor.to_string(),
            action: "test-action".to_string(),
            resource: "test-resource".to_string(),
            outcome,
            details: None,
        }
    }

    #[test]
    fn test_audit_log_record_single_event() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "user1",
            AuditOutcome::Success,
        ));
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries()[0].actor, "user1");
    }

    #[test]
    fn test_audit_log_rotates_oldest_when_full() {
        let mut log = AuditLog::new(3);
        for i in 0..5 {
            log.record_event(
                AuditCategory::Authorization,
                AuditSeverity::Info,
                format!("actor-{i}"),
                "action",
                "resource",
                AuditOutcome::Success,
                None,
            );
        }
        assert_eq!(log.len(), 3);
        // oldest two (actor-0, actor-1) should have been evicted
        let actors: Vec<&str> = log.entries().iter().map(|e| e.actor.as_str()).collect();
        assert_eq!(actors, vec!["actor-2", "actor-3", "actor-4"]);
    }

    #[test]
    fn test_audit_log_filter_by_category() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Info,
            "sys",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "sys",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Warning,
            "sys",
            AuditOutcome::Failure,
        ));
        let plugin_entries = log.entries_by_category(AuditCategory::PluginLoad);
        assert_eq!(plugin_entries.len(), 2);
    }

    #[test]
    fn test_audit_log_filter_by_severity() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::ConfigChange,
            AuditSeverity::Info,
            "a",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::ConfigChange,
            AuditSeverity::Warning,
            "a",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::ConfigChange,
            AuditSeverity::Critical,
            "a",
            AuditOutcome::Success,
        ));
        let warnings_and_above = log.entries_by_severity(AuditSeverity::Warning);
        assert_eq!(warnings_and_above.len(), 2);
        assert!(
            warnings_and_above
                .iter()
                .all(|e| e.severity >= AuditSeverity::Warning)
        );
    }

    #[test]
    fn test_audit_log_filter_by_actor() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::DeviceAccess,
            AuditSeverity::Info,
            "alice",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::DeviceAccess,
            AuditSeverity::Info,
            "bob",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::FileAccess,
            AuditSeverity::Info,
            "alice",
            AuditOutcome::Denied,
        ));
        assert_eq!(log.entries_by_actor("alice").len(), 2);
        assert_eq!(log.entries_by_actor("bob").len(), 1);
    }

    #[test]
    fn test_audit_log_failures_filter() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "u",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Warning,
            "u",
            AuditOutcome::Failure,
        ));
        log.record(make_entry(
            AuditCategory::Authorization,
            AuditSeverity::Alert,
            "u",
            AuditOutcome::Denied,
        ));
        let failures = log.failures();
        assert_eq!(failures.len(), 2);
        assert!(failures.iter().all(|e| e.outcome != AuditOutcome::Success));
    }

    #[test]
    fn test_audit_log_recent() {
        let mut log = AuditLog::new(10);
        for i in 0..5 {
            log.record_event(
                AuditCategory::ServiceLifecycle,
                AuditSeverity::Info,
                format!("svc-{i}"),
                "start",
                "service",
                AuditOutcome::Success,
                None,
            );
        }
        let last_two = log.recent(2);
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].actor, "svc-3");
        assert_eq!(last_two[1].actor, "svc-4");
    }

    #[test]
    fn test_audit_log_enable_disable() {
        let mut log = AuditLog::new(10);
        assert!(log.is_enabled());
        log.disable();
        assert!(!log.is_enabled());
        log.enable();
        assert!(log.is_enabled());
    }

    #[test]
    fn test_audit_log_disabled_does_not_record() {
        let mut log = AuditLog::new(10);
        log.disable();
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "u",
            AuditOutcome::Success,
        ));
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_audit_log_export_json_valid() {
        let mut log = AuditLog::new(10);
        log.record_event(
            AuditCategory::PluginLoad,
            AuditSeverity::Info,
            "system",
            "load",
            "my-plugin",
            AuditOutcome::Success,
            Some("loaded v1.0".to_string()),
        );
        let json = log.export_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let arr = parsed.as_array().expect("should be array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["category"], "PluginLoad");
        assert_eq!(arr[0]["outcome"], "Success");
        assert_eq!(arr[0]["details"], "loaded v1.0");
    }

    #[test]
    fn test_audit_log_clear() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::UpdateInstall,
            AuditSeverity::Info,
            "updater",
            AuditOutcome::Success,
        ));
        assert_eq!(log.len(), 1);
        log.clear();
        assert_eq!(log.len(), 0);
        assert!(log.entries().is_empty());
    }

    #[test]
    fn test_audit_log_multiple_categories() {
        let mut log = AuditLog::new(20);
        let categories = [
            AuditCategory::Authentication,
            AuditCategory::Authorization,
            AuditCategory::ConfigChange,
            AuditCategory::PluginLoad,
            AuditCategory::DeviceAccess,
            AuditCategory::NetworkAccess,
        ];
        for cat in &categories {
            log.record(make_entry(
                *cat,
                AuditSeverity::Info,
                "multi",
                AuditOutcome::Success,
            ));
        }
        assert_eq!(log.len(), categories.len());
        for cat in &categories {
            assert_eq!(log.entries_by_category(*cat).len(), 1);
        }
    }

    // --- AuditFilter / query tests ---

    #[test]
    fn test_query_empty_filter_returns_all() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "u",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Critical,
            "v",
            AuditOutcome::Failure,
        ));
        let filter = AuditFilter::new();
        assert_eq!(log.query(&filter).len(), 2);
    }

    #[test]
    fn test_query_filter_by_severity() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::ConfigChange,
            AuditSeverity::Info,
            "a",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::ConfigChange,
            AuditSeverity::Alert,
            "a",
            AuditOutcome::Success,
        ));
        let filter = AuditFilter::new().with_min_severity(AuditSeverity::Alert);
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, AuditSeverity::Alert);
    }

    #[test]
    fn test_query_filter_by_category() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Info,
            "x",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::FileAccess,
            AuditSeverity::Info,
            "x",
            AuditOutcome::Success,
        ));
        let filter = AuditFilter::new().with_category(AuditCategory::FileAccess);
        assert_eq!(log.query(&filter).len(), 1);
    }

    #[test]
    fn test_query_filter_by_actor() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "alice",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Info,
            "bob",
            AuditOutcome::Success,
        ));
        let filter = AuditFilter::new().with_actor("bob");
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].actor, "bob");
    }

    #[test]
    fn test_query_combined_filters() {
        let mut log = AuditLog::new(10);
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Info,
            "sys",
            AuditOutcome::Success,
        ));
        log.record(make_entry(
            AuditCategory::PluginLoad,
            AuditSeverity::Critical,
            "sys",
            AuditOutcome::Failure,
        ));
        log.record(make_entry(
            AuditCategory::Authentication,
            AuditSeverity::Critical,
            "sys",
            AuditOutcome::Denied,
        ));
        let filter = AuditFilter::new()
            .with_category(AuditCategory::PluginLoad)
            .with_min_severity(AuditSeverity::Critical);
        let results = log.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, AuditCategory::PluginLoad);
        assert_eq!(results[0].severity, AuditSeverity::Critical);
    }

    #[test]
    fn test_query_filter_by_time_range() {
        use std::time::Duration;

        let mut log = AuditLog::new(10);
        let now = SystemTime::now();
        let past = now - Duration::from_secs(60);
        let future = now + Duration::from_secs(60);

        // Record an entry with known timestamp
        log.record(AuditEntry {
            timestamp: now,
            category: AuditCategory::ServiceLifecycle,
            severity: AuditSeverity::Info,
            actor: "timer".to_string(),
            action: "tick".to_string(),
            resource: "clock".to_string(),
            outcome: AuditOutcome::Success,
            details: None,
        });

        // Filter that covers the entry
        let filter = AuditFilter::new().with_time_range(past, future);
        assert_eq!(log.query(&filter).len(), 1);

        // Filter that excludes the entry (range in the past)
        let far_past = past - Duration::from_secs(120);
        let filter_old = AuditFilter::new().with_time_range(far_past, past);
        assert_eq!(log.query(&filter_old).len(), 0);
    }
}
