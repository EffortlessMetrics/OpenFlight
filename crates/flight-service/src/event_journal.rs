// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Persistent event journal for the flight service.
//!
//! Records important service events in a ring-buffer with configurable
//! maximum size and automatic rotation of oldest entries.

use std::collections::VecDeque;
use std::time::SystemTime;

/// An event entry in the journal.
#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub timestamp: SystemTime,
    pub level: JournalLevel,
    pub category: EventCategory,
    pub message: String,
    pub details: Option<String>,
}

/// Severity level for journal entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// Category of a service event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    DeviceConnected,
    DeviceDisconnected,
    ProfileLoaded,
    ProfileError,
    SimConnected,
    SimDisconnected,
    AxisCalibration,
    FfbEvent,
    ServiceStartup,
    ServiceShutdown,
    PluginEvent,
    ConfigChange,
    SafeMode,
    WatchdogAlert,
}

/// Aggregated counts for a journal snapshot.
#[derive(Debug, Clone, Default)]
pub struct JournalSummary {
    pub total: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub critical_count: usize,
    pub category_counts: std::collections::HashMap<EventCategory, usize>,
}

/// Ring-buffer event journal with configurable max size.
pub struct EventJournal {
    entries: VecDeque<JournalEntry>,
    max_entries: usize,
}

impl EventJournal {
    /// Create a new journal with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    /// Record an event, rotating the oldest entry if the journal is full.
    pub fn record(
        &mut self,
        level: JournalLevel,
        category: EventCategory,
        message: impl Into<String>,
        details: Option<String>,
    ) {
        if self.entries.len() == self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(JournalEntry {
            timestamp: SystemTime::now(),
            level,
            category,
            message: message.into(),
            details,
        });
    }

    /// Return a reference to all entries.
    pub fn entries(&self) -> &VecDeque<JournalEntry> {
        &self.entries
    }

    /// Return entries recorded at or after `since`.
    pub fn entries_since(&self, since: SystemTime) -> Vec<&JournalEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= since)
            .collect()
    }

    /// Return entries matching a specific category.
    pub fn entries_by_category(&self, category: EventCategory) -> Vec<&JournalEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Return entries at or above the given severity level.
    pub fn entries_by_level(&self, level: JournalLevel) -> Vec<&JournalEntry> {
        self.entries.iter().filter(|e| e.level == level).collect()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the journal is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The most recently recorded entry, if any.
    pub fn last_entry(&self) -> Option<&JournalEntry> {
        self.entries.back()
    }

    /// Produce an aggregate summary of counts per level and category.
    pub fn summary(&self) -> JournalSummary {
        let mut s = JournalSummary {
            total: self.entries.len(),
            ..JournalSummary::default()
        };
        for entry in &self.entries {
            match entry.level {
                JournalLevel::Info => s.info_count += 1,
                JournalLevel::Warning => s.warning_count += 1,
                JournalLevel::Error => s.error_count += 1,
                JournalLevel::Critical => s.critical_count += 1,
            }
            *s.category_counts.entry(entry.category).or_insert(0) += 1;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn record_and_retrieve_single_entry() {
        let mut journal = EventJournal::new(10);
        journal.record(
            JournalLevel::Info,
            EventCategory::ServiceStartup,
            "Service started",
            None,
        );
        assert_eq!(journal.len(), 1);
        let entry = journal.last_entry().unwrap();
        assert_eq!(entry.level, JournalLevel::Info);
        assert_eq!(entry.category, EventCategory::ServiceStartup);
        assert_eq!(entry.message, "Service started");
        assert!(entry.details.is_none());
    }

    #[test]
    fn record_multiple_entries_in_order() {
        let mut journal = EventJournal::new(10);
        journal.record(
            JournalLevel::Info,
            EventCategory::ServiceStartup,
            "first",
            None,
        );
        journal.record(
            JournalLevel::Warning,
            EventCategory::DeviceConnected,
            "second",
            None,
        );
        journal.record(
            JournalLevel::Error,
            EventCategory::ProfileError,
            "third",
            None,
        );

        let entries = journal.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "first");
        assert_eq!(entries[1].message, "second");
        assert_eq!(entries[2].message, "third");
    }

    #[test]
    fn rotation_when_max_exceeded() {
        let mut journal = EventJournal::new(3);
        for i in 0..5 {
            journal.record(
                JournalLevel::Info,
                EventCategory::PluginEvent,
                format!("event-{i}"),
                None,
            );
        }
        assert_eq!(journal.len(), 3);
        let entries = journal.entries();
        assert_eq!(entries[0].message, "event-2");
        assert_eq!(entries[1].message, "event-3");
        assert_eq!(entries[2].message, "event-4");
    }

    #[test]
    fn filter_by_category() {
        let mut journal = EventJournal::new(10);
        journal.record(
            JournalLevel::Info,
            EventCategory::DeviceConnected,
            "dev1",
            None,
        );
        journal.record(
            JournalLevel::Info,
            EventCategory::ProfileLoaded,
            "profile",
            None,
        );
        journal.record(
            JournalLevel::Info,
            EventCategory::DeviceConnected,
            "dev2",
            None,
        );

        let filtered = journal.entries_by_category(EventCategory::DeviceConnected);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "dev1");
        assert_eq!(filtered[1].message, "dev2");
    }

    #[test]
    fn filter_by_level() {
        let mut journal = EventJournal::new(10);
        journal.record(JournalLevel::Info, EventCategory::PluginEvent, "info", None);
        journal.record(
            JournalLevel::Error,
            EventCategory::ProfileError,
            "error1",
            None,
        );
        journal.record(JournalLevel::Error, EventCategory::FfbEvent, "error2", None);
        journal.record(
            JournalLevel::Critical,
            EventCategory::WatchdogAlert,
            "crit",
            None,
        );

        let errors = journal.entries_by_level(JournalLevel::Error);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].message, "error1");
        assert_eq!(errors[1].message, "error2");
    }

    #[test]
    fn filter_by_time_entries_since() {
        let mut journal = EventJournal::new(10);
        journal.record(
            JournalLevel::Info,
            EventCategory::ServiceStartup,
            "old",
            None,
        );

        // Small sleep so the next entries have a later timestamp.
        thread::sleep(Duration::from_millis(20));
        let cutoff = SystemTime::now();
        thread::sleep(Duration::from_millis(20));

        journal.record(
            JournalLevel::Info,
            EventCategory::SimConnected,
            "new1",
            None,
        );
        journal.record(
            JournalLevel::Warning,
            EventCategory::ConfigChange,
            "new2",
            None,
        );

        let recent = journal.entries_since(cutoff);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "new1");
        assert_eq!(recent[1].message, "new2");
    }

    #[test]
    fn clear_empties_the_journal() {
        let mut journal = EventJournal::new(10);
        journal.record(JournalLevel::Info, EventCategory::PluginEvent, "a", None);
        journal.record(JournalLevel::Info, EventCategory::PluginEvent, "b", None);
        assert_eq!(journal.len(), 2);

        journal.clear();
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);
        assert!(journal.last_entry().is_none());
    }

    #[test]
    fn summary_counts_correctly() {
        let mut journal = EventJournal::new(20);
        journal.record(
            JournalLevel::Info,
            EventCategory::DeviceConnected,
            "i1",
            None,
        );
        journal.record(
            JournalLevel::Info,
            EventCategory::DeviceConnected,
            "i2",
            None,
        );
        journal.record(
            JournalLevel::Warning,
            EventCategory::ProfileError,
            "w1",
            None,
        );
        journal.record(JournalLevel::Error, EventCategory::FfbEvent, "e1", None);
        journal.record(
            JournalLevel::Critical,
            EventCategory::WatchdogAlert,
            "c1",
            None,
        );

        let s = journal.summary();
        assert_eq!(s.total, 5);
        assert_eq!(s.info_count, 2);
        assert_eq!(s.warning_count, 1);
        assert_eq!(s.error_count, 1);
        assert_eq!(s.critical_count, 1);
        assert_eq!(s.category_counts[&EventCategory::DeviceConnected], 2);
        assert_eq!(s.category_counts[&EventCategory::WatchdogAlert], 1);
    }

    #[test]
    fn last_entry_returns_most_recent() {
        let mut journal = EventJournal::new(10);
        journal.record(
            JournalLevel::Info,
            EventCategory::AxisCalibration,
            "first",
            None,
        );
        journal.record(
            JournalLevel::Warning,
            EventCategory::SafeMode,
            "latest",
            Some("detail".into()),
        );

        let last = journal.last_entry().unwrap();
        assert_eq!(last.message, "latest");
        assert_eq!(last.level, JournalLevel::Warning);
        assert_eq!(last.details.as_deref(), Some("detail"));
    }

    #[test]
    fn empty_journal_edge_cases() {
        let journal = EventJournal::new(5);
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);
        assert!(journal.last_entry().is_none());
        assert!(journal.entries().is_empty());
        assert!(journal.entries_since(SystemTime::now()).is_empty());
        assert!(
            journal
                .entries_by_category(EventCategory::ServiceStartup)
                .is_empty()
        );
        assert!(journal.entries_by_level(JournalLevel::Error).is_empty());

        let s = journal.summary();
        assert_eq!(s.total, 0);
        assert_eq!(s.info_count, 0);
        assert!(s.category_counts.is_empty());
    }

    #[test]
    fn details_field_preserved() {
        let mut journal = EventJournal::new(5);
        journal.record(
            JournalLevel::Error,
            EventCategory::ProfileError,
            "load failed",
            Some("file not found: default.yaml".into()),
        );
        let entry = journal.last_entry().unwrap();
        assert_eq!(
            entry.details.as_deref(),
            Some("file not found: default.yaml")
        );
    }

    #[test]
    fn rotation_preserves_newest_entries() {
        let mut journal = EventJournal::new(2);
        journal.record(JournalLevel::Info, EventCategory::ServiceStartup, "a", None);
        journal.record(JournalLevel::Info, EventCategory::SimConnected, "b", None);
        journal.record(
            JournalLevel::Warning,
            EventCategory::SimDisconnected,
            "c",
            None,
        );

        assert_eq!(journal.len(), 2);
        assert_eq!(journal.entries()[0].message, "b");
        assert_eq!(journal.entries()[1].message, "c");
    }
}
