//! Capability-based security for the plugin system.
//!
//! Each plugin declares the capabilities it needs in its manifest. The runtime
//! enforces that plugins only access resources they have been granted.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Individual capabilities a plugin may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadAxes,
    WriteAxes,
    ReadButtons,
    WriteButtons,
    ReadTelemetry,
    WriteFfb,
    ReadProfile,
    /// Restricted — requires explicit user consent.
    AccessNetwork,
}

impl Capability {
    /// Returns the bit position for this capability.
    const fn bit(self) -> u16 {
        match self {
            Self::ReadAxes => 0,
            Self::WriteAxes => 1,
            Self::ReadButtons => 2,
            Self::WriteButtons => 3,
            Self::ReadTelemetry => 4,
            Self::WriteFfb => 5,
            Self::ReadProfile => 6,
            Self::AccessNetwork => 7,
        }
    }

    /// All defined capabilities, for iteration.
    pub const ALL: &[Capability] = &[
        Self::ReadAxes,
        Self::WriteAxes,
        Self::ReadButtons,
        Self::WriteButtons,
        Self::ReadTelemetry,
        Self::WriteFfb,
        Self::ReadProfile,
        Self::AccessNetwork,
    ];
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ReadAxes => "read_axes",
            Self::WriteAxes => "write_axes",
            Self::ReadButtons => "read_buttons",
            Self::WriteButtons => "write_buttons",
            Self::ReadTelemetry => "read_telemetry",
            Self::WriteFfb => "write_ffb",
            Self::ReadProfile => "read_profile",
            Self::AccessNetwork => "access_network",
        };
        f.write_str(s)
    }
}

/// A compact bitflag set of [`Capability`] values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CapabilitySet(u16);

impl CapabilitySet {
    /// An empty set with no capabilities.
    pub const EMPTY: Self = Self(0);

    /// Create a set from an iterator of capabilities.
    pub fn from_caps(iter: impl IntoIterator<Item = Capability>) -> Self {
        let mut bits = 0u16;
        for cap in iter {
            bits |= 1 << cap.bit();
        }
        Self(bits)
    }

    /// Insert a capability into the set.
    pub fn insert(&mut self, cap: Capability) {
        self.0 |= 1 << cap.bit();
    }

    /// Check whether a capability is present.
    pub fn contains(self, cap: Capability) -> bool {
        (self.0 & (1 << cap.bit())) != 0
    }

    /// Check whether this set is a superset of `other`.
    pub fn contains_all(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Return capabilities in `other` that are missing from `self`.
    pub fn missing(self, requested: Self) -> Vec<Capability> {
        Capability::ALL
            .iter()
            .copied()
            .filter(|c| requested.contains(*c) && !self.contains(*c))
            .collect()
    }

    /// Return the number of capabilities in this set.
    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Return true if the set is empty.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Iterate over the capabilities in this set.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .iter()
            .copied()
            .filter(move |c| self.contains(*c))
    }
}

impl fmt::Display for CapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let caps: Vec<String> = self.iter().map(|c| c.to_string()).collect();
        write!(f, "{{{}}}", caps.join(", "))
    }
}

/// Error returned when a capability check fails.
#[derive(Debug, Clone, thiserror::Error)]
#[error("capability denied: plugin requires {denied} but only granted {granted}")]
pub struct CapabilityDenied {
    /// The denied capabilities.
    pub denied: CapabilitySet,
    /// The granted set.
    pub granted: CapabilitySet,
}

/// Checks plugin capability requests against their manifest grants.
pub struct CapabilityChecker;

impl CapabilityChecker {
    /// Check that all `requested` capabilities are present in the `granted` set.
    ///
    /// Returns `Ok(())` if granted is a superset, or `Err(CapabilityDenied)` listing
    /// the missing capabilities. Denied attempts are logged at WARN level.
    pub fn check(granted: CapabilitySet, requested: CapabilitySet) -> Result<(), CapabilityDenied> {
        if granted.contains_all(requested) {
            Ok(())
        } else {
            let missing = granted.missing(requested);
            let denied = CapabilitySet::from_caps(missing.iter().copied());
            tracing::warn!(
                granted = %granted,
                requested = %requested,
                denied = %denied,
                "plugin capability access denied"
            );
            Err(CapabilityDenied { denied, granted })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set() {
        let set = CapabilitySet::EMPTY;
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(!set.contains(Capability::ReadAxes));
    }

    #[test]
    fn insert_and_contains() {
        let mut set = CapabilitySet::EMPTY;
        set.insert(Capability::ReadAxes);
        set.insert(Capability::WriteFfb);
        assert!(set.contains(Capability::ReadAxes));
        assert!(set.contains(Capability::WriteFfb));
        assert!(!set.contains(Capability::AccessNetwork));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn from_caps_builds_set() {
        let set = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::ReadButtons,
            Capability::ReadTelemetry,
        ]);
        assert_eq!(set.len(), 3);
        assert!(set.contains(Capability::ReadAxes));
        assert!(set.contains(Capability::ReadButtons));
        assert!(set.contains(Capability::ReadTelemetry));
    }

    #[test]
    fn contains_all_subset() {
        let granted = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::WriteAxes,
            Capability::ReadTelemetry,
        ]);
        let requested = CapabilitySet::from_caps([Capability::ReadAxes, Capability::ReadTelemetry]);
        assert!(granted.contains_all(requested));
    }

    #[test]
    fn contains_all_fails_for_superset() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let requested = CapabilitySet::from_caps([Capability::ReadAxes, Capability::WriteAxes]);
        assert!(!granted.contains_all(requested));
    }

    #[test]
    fn missing_returns_diff() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let requested = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::WriteFfb,
            Capability::AccessNetwork,
        ]);
        let missing = granted.missing(requested);
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&Capability::WriteFfb));
        assert!(missing.contains(&Capability::AccessNetwork));
    }

    #[test]
    fn checker_allows_valid_request() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes, Capability::ReadTelemetry]);
        let requested = CapabilitySet::from_caps([Capability::ReadAxes]);
        assert!(CapabilityChecker::check(granted, requested).is_ok());
    }

    #[test]
    fn checker_denies_excess_capabilities() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let requested = CapabilitySet::from_caps([Capability::ReadAxes, Capability::AccessNetwork]);
        let err = CapabilityChecker::check(granted, requested).unwrap_err();
        assert!(err.denied.contains(Capability::AccessNetwork));
        assert!(!err.denied.contains(Capability::ReadAxes));
    }

    #[test]
    fn checker_allows_empty_request() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let requested = CapabilitySet::EMPTY;
        assert!(CapabilityChecker::check(granted, requested).is_ok());
    }

    #[test]
    fn checker_denies_all_when_empty_grant() {
        let granted = CapabilitySet::EMPTY;
        let requested = CapabilitySet::from_caps([Capability::ReadAxes, Capability::WriteAxes]);
        let err = CapabilityChecker::check(granted, requested).unwrap_err();
        assert_eq!(err.denied.len(), 2);
    }

    #[test]
    fn display_capability() {
        assert_eq!(Capability::ReadAxes.to_string(), "read_axes");
        assert_eq!(Capability::AccessNetwork.to_string(), "access_network");
    }

    #[test]
    fn display_set() {
        let set = CapabilitySet::from_caps([Capability::ReadAxes, Capability::WriteFfb]);
        let s = set.to_string();
        assert!(s.contains("read_axes"));
        assert!(s.contains("write_ffb"));
    }

    #[test]
    fn iter_capabilities() {
        let set = CapabilitySet::from_caps([Capability::ReadButtons, Capability::ReadProfile]);
        let collected: Vec<_> = set.iter().collect();
        assert_eq!(collected.len(), 2);
        assert!(collected.contains(&Capability::ReadButtons));
        assert!(collected.contains(&Capability::ReadProfile));
    }
}
