// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile comparison and diffing.
//!
//! Flattens profile JSON into dotted key-value maps and produces a
//! structured diff that can be filtered or rendered as text.

use std::collections::BTreeMap;

/// Represents a difference between two profile values.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffEntry {
    Added {
        key: String,
        value: String,
    },
    Removed {
        key: String,
        value: String,
    },
    Changed {
        key: String,
        old_value: String,
        new_value: String,
    },
}

/// Result of comparing two profiles.
#[derive(Debug, Clone)]
pub struct ProfileDiff {
    pub left_name: String,
    pub right_name: String,
    pub differences: Vec<DiffEntry>,
}

/// Flat key-value representation of a profile for comparison.
pub type ProfileMap = BTreeMap<String, String>;

/// Recursively flatten a JSON value into dotted key paths.
pub fn flatten_profile(profile: &serde_json::Value, prefix: &str) -> ProfileMap {
    let mut map = ProfileMap::new();
    flatten_recursive(profile, prefix, &mut map);
    map
}

fn flatten_recursive(value: &serde_json::Value, prefix: &str, map: &mut ProfileMap) {
    match value {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_recursive(v, &key, map);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let key = if prefix.is_empty() {
                    format!("{i}")
                } else {
                    format!("{prefix}[{i}]")
                };
                flatten_recursive(v, &key, map);
            }
        }
        _ => {
            let text = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            map.insert(prefix.to_owned(), text);
        }
    }
}

/// Compare two flattened profile maps and return a [`ProfileDiff`].
pub fn compare_profiles(
    left: &ProfileMap,
    right: &ProfileMap,
    left_name: &str,
    right_name: &str,
) -> ProfileDiff {
    let mut differences = Vec::new();

    for (key, left_val) in left {
        match right.get(key) {
            Some(right_val) if right_val != left_val => {
                differences.push(DiffEntry::Changed {
                    key: key.clone(),
                    old_value: left_val.clone(),
                    new_value: right_val.clone(),
                });
            }
            None => {
                differences.push(DiffEntry::Removed {
                    key: key.clone(),
                    value: left_val.clone(),
                });
            }
            _ => {}
        }
    }

    for (key, right_val) in right {
        if !left.contains_key(key) {
            differences.push(DiffEntry::Added {
                key: key.clone(),
                value: right_val.clone(),
            });
        }
    }

    ProfileDiff {
        left_name: left_name.to_owned(),
        right_name: right_name.to_owned(),
        differences,
    }
}

impl ProfileDiff {
    /// Returns `true` when the two profiles are identical.
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }

    /// Number of keys present only in the right profile.
    pub fn added_count(&self) -> usize {
        self.differences
            .iter()
            .filter(|d| matches!(d, DiffEntry::Added { .. }))
            .count()
    }

    /// Number of keys present only in the left profile.
    pub fn removed_count(&self) -> usize {
        self.differences
            .iter()
            .filter(|d| matches!(d, DiffEntry::Removed { .. }))
            .count()
    }

    /// Number of keys whose values differ.
    pub fn changed_count(&self) -> usize {
        self.differences
            .iter()
            .filter(|d| matches!(d, DiffEntry::Changed { .. }))
            .count()
    }

    /// Total number of differences.
    pub fn total_changes(&self) -> usize {
        self.differences.len()
    }

    /// Human-readable text report.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Profile diff: {} vs {}\n",
            self.left_name, self.right_name
        ));
        out.push_str(&format!(
            "Changes: {} added, {} removed, {} changed\n",
            self.added_count(),
            self.removed_count(),
            self.changed_count(),
        ));

        for d in &self.differences {
            match d {
                DiffEntry::Added { key, value } => {
                    out.push_str(&format!("  + {key}: {value}\n"));
                }
                DiffEntry::Removed { key, value } => {
                    out.push_str(&format!("  - {key}: {value}\n"));
                }
                DiffEntry::Changed {
                    key,
                    old_value,
                    new_value,
                } => {
                    out.push_str(&format!("  ~ {key}: {old_value} -> {new_value}\n"));
                }
            }
        }
        out
    }

    /// Return a new diff containing only entries whose key starts with `prefix`.
    pub fn filter_by_prefix(&self, prefix: &str) -> ProfileDiff {
        let filtered = self
            .differences
            .iter()
            .filter(|d| {
                let key = match d {
                    DiffEntry::Added { key, .. }
                    | DiffEntry::Removed { key, .. }
                    | DiffEntry::Changed { key, .. } => key,
                };
                key.starts_with(prefix)
            })
            .cloned()
            .collect();

        ProfileDiff {
            left_name: self.left_name.clone(),
            right_name: self.right_name.clone(),
            differences: filtered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_identical_profiles_empty_diff() {
        let val = json!({"axes": {"pitch": {"deadzone": 0.03}}});
        let left = flatten_profile(&val, "");
        let right = flatten_profile(&val, "");
        let diff = compare_profiles(&left, &right, "a", "b");
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_added_keys_detected() {
        let left = ProfileMap::new();
        let mut right = ProfileMap::new();
        right.insert("axes.roll".into(), "0.5".into());

        let diff = compare_profiles(&left, &right, "a", "b");
        assert_eq!(diff.added_count(), 1);
        assert!(matches!(&diff.differences[0], DiffEntry::Added { key, .. } if key == "axes.roll"));
    }

    #[test]
    fn test_removed_keys_detected() {
        let mut left = ProfileMap::new();
        left.insert("axes.yaw".into(), "0.1".into());
        let right = ProfileMap::new();

        let diff = compare_profiles(&left, &right, "a", "b");
        assert_eq!(diff.removed_count(), 1);
        assert!(
            matches!(&diff.differences[0], DiffEntry::Removed { key, .. } if key == "axes.yaw")
        );
    }

    #[test]
    fn test_changed_values_detected() {
        let mut left = ProfileMap::new();
        left.insert("axes.pitch.deadzone".into(), "0.03".into());
        let mut right = ProfileMap::new();
        right.insert("axes.pitch.deadzone".into(), "0.05".into());

        let diff = compare_profiles(&left, &right, "a", "b");
        assert_eq!(diff.changed_count(), 1);
        assert!(matches!(
            &diff.differences[0],
            DiffEntry::Changed { key, old_value, new_value }
                if key == "axes.pitch.deadzone" && old_value == "0.03" && new_value == "0.05"
        ));
    }

    #[test]
    fn test_nested_object_flattening() {
        let val = json!({"a": {"b": {"c": 42}}});
        let map = flatten_profile(&val, "");
        assert_eq!(map.get("a.b.c").unwrap(), "42");
    }

    #[test]
    fn test_array_flattening() {
        let val = json!({"detents": [0.25, 0.5, 0.75]});
        let map = flatten_profile(&val, "");
        assert_eq!(map.get("detents[0]").unwrap(), "0.25");
        assert_eq!(map.get("detents[1]").unwrap(), "0.5");
        assert_eq!(map.get("detents[2]").unwrap(), "0.75");
    }

    #[test]
    fn test_mixed_changes() {
        let mut left = ProfileMap::new();
        left.insert("keep".into(), "same".into());
        left.insert("remove_me".into(), "old".into());
        left.insert("change_me".into(), "v1".into());

        let mut right = ProfileMap::new();
        right.insert("keep".into(), "same".into());
        right.insert("add_me".into(), "new".into());
        right.insert("change_me".into(), "v2".into());

        let diff = compare_profiles(&left, &right, "a", "b");
        assert_eq!(diff.added_count(), 1);
        assert_eq!(diff.removed_count(), 1);
        assert_eq!(diff.changed_count(), 1);
        assert_eq!(diff.total_changes(), 3);
    }

    #[test]
    fn test_filter_by_prefix() {
        let mut left = ProfileMap::new();
        left.insert("axes.pitch".into(), "0.1".into());
        left.insert("meta.name".into(), "old".into());

        let mut right = ProfileMap::new();
        right.insert("axes.pitch".into(), "0.2".into());
        right.insert("meta.name".into(), "new".into());

        let diff = compare_profiles(&left, &right, "a", "b");
        let axes_only = diff.filter_by_prefix("axes");
        assert_eq!(axes_only.total_changes(), 1);
        assert!(matches!(
            &axes_only.differences[0],
            DiffEntry::Changed { key, .. } if key == "axes.pitch"
        ));
    }

    #[test]
    fn test_text_report_includes_all_changes() {
        let mut left = ProfileMap::new();
        left.insert("removed".into(), "val".into());
        left.insert("changed".into(), "old".into());

        let mut right = ProfileMap::new();
        right.insert("added".into(), "val".into());
        right.insert("changed".into(), "new".into());

        let diff = compare_profiles(&left, &right, "base", "override");
        let text = diff.to_text();

        assert!(text.contains("base vs override"));
        assert!(text.contains("+ added"));
        assert!(text.contains("- removed"));
        assert!(text.contains("~ changed: old -> new"));
    }

    #[test]
    fn test_empty_profiles_empty_diff() {
        let left = flatten_profile(&json!({}), "");
        let right = flatten_profile(&json!({}), "");
        let diff = compare_profiles(&left, &right, "a", "b");
        assert!(diff.is_empty());
    }

    #[test]
    fn test_string_values_not_quoted() {
        let val = json!({"sim": "msfs"});
        let map = flatten_profile(&val, "");
        assert_eq!(map.get("sim").unwrap(), "msfs");
    }

    #[test]
    fn test_flatten_with_prefix() {
        let val = json!({"x": 1});
        let map = flatten_profile(&val, "root");
        assert_eq!(map.get("root.x").unwrap(), "1");
    }
}
