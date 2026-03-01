// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin capability sandboxing (REQ-931).
//!
//! Provides a bitflag-based capability model for plugin sandboxing. Each plugin
//! declares a [`CapabilitySet`] in its manifest; the host enforces that the
//! plugin never exercises capabilities beyond what was granted.

use std::fmt;

use crate::SecurityError;

// ---------------------------------------------------------------------------
// Capability flags
// ---------------------------------------------------------------------------

/// Individual capability flags.
///
/// Each constant is a single bit so that sets can be combined with bitwise OR
/// and checked with bitwise AND.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability(u32);

impl Capability {
    pub const READ_TELEMETRY: Self = Self(1 << 0);
    pub const WRITE_CONTROLS: Self = Self(1 << 1);
    pub const ACCESS_HID: Self = Self(1 << 2);
    pub const NETWORK_IO: Self = Self(1 << 3);
    pub const FILE_IO: Self = Self(1 << 4);
}

impl Capability {
    /// All defined capabilities for iteration / display.
    const ALL: &'static [(Self, &'static str)] = &[
        (Self::READ_TELEMETRY, "ReadTelemetry"),
        (Self::WRITE_CONTROLS, "WriteControls"),
        (Self::ACCESS_HID, "AccessHid"),
        (Self::NETWORK_IO, "NetworkIO"),
        (Self::FILE_IO, "FileIO"),
    ];

    /// Bitmask of all known capability bits.
    const KNOWN_BITS: u32 = Self::READ_TELEMETRY.0
        | Self::WRITE_CONTROLS.0
        | Self::ACCESS_HID.0
        | Self::NETWORK_IO.0
        | Self::FILE_IO.0;

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(c, _)| *c == self)
            .map(|(_, l)| *l)
            .unwrap_or("Unknown")
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// CapabilitySet — bitflag set
// ---------------------------------------------------------------------------

/// A set of capabilities represented as a bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySet(u32);

impl CapabilitySet {
    /// Empty set — no capabilities.
    pub const NONE: Self = Self(0);

    /// Create a set from a single capability.
    pub fn from_single(cap: Capability) -> Self {
        Self(cap.0)
    }

    /// Create a set from a raw bitmask, stripping any unknown bits.
    pub fn from_bits(bits: u32) -> Self {
        Self(bits & Capability::KNOWN_BITS)
    }

    /// Create a set from a raw bitmask, rejecting unknown bits.
    pub fn from_bits_strict(bits: u32) -> Result<Self, UnknownCapabilityBits> {
        let unknown = bits & !Capability::KNOWN_BITS;
        if unknown != 0 {
            Err(UnknownCapabilityBits(unknown))
        } else {
            Ok(Self(bits))
        }
    }

    /// Return the raw bitmask.
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Test whether a single capability is present.
    pub fn contains(self, cap: Capability) -> bool {
        self.0 & cap.0 != 0
    }

    /// Union of two sets.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection of two sets.
    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Capabilities present in `self` but not in `other`.
    pub fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Returns `true` when this set is a subset of (or equal to) `other`.
    pub fn is_subset_of(self, other: Self) -> bool {
        self.difference(other).0 == 0
    }

    /// Returns `true` when the set contains no capabilities.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Iterate over the individual capabilities that are set.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .iter()
            .filter(move |(c, _)| self.contains(*c))
            .map(|(c, _)| *c)
    }
}

/// Error returned by [`CapabilitySet::from_bits_strict`] when unknown bits are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownCapabilityBits(pub u32);

impl fmt::Display for UnknownCapabilityBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown capability bits: {:#010x}", self.0)
    }
}

impl std::error::Error for UnknownCapabilityBits {}

impl std::ops::BitOr for CapabilitySet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitAnd for CapabilitySet {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.intersection(rhs)
    }
}

impl fmt::Display for CapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let labels: Vec<&str> = self.iter().map(|c| c.label()).collect();
        if labels.is_empty() {
            f.write_str("(none)")
        } else {
            f.write_str(&labels.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// CapabilityViolation
// ---------------------------------------------------------------------------

/// Details about a denied capability request.
#[derive(Debug, Clone)]
pub struct CapabilityViolation {
    /// The capabilities that were requested but not allowed.
    pub denied: CapabilitySet,
    /// Human-readable description of the violation.
    pub message: String,
}

impl fmt::Display for CapabilityViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capability violation: {}", self.message)
    }
}

// ---------------------------------------------------------------------------
// SandboxPolicy
// ---------------------------------------------------------------------------

/// Declares which capabilities a plugin is *allowed* to use.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// The set of capabilities the policy grants.
    pub allowed: CapabilitySet,
    /// Optional label for debugging / audit purposes.
    pub label: String,
}

impl SandboxPolicy {
    /// Create a new policy.
    pub fn new(label: impl Into<String>, allowed: CapabilitySet) -> Self {
        Self {
            allowed,
            label: label.into(),
        }
    }

    /// A maximally restrictive policy — no capabilities.
    pub fn deny_all(label: impl Into<String>) -> Self {
        Self::new(label, CapabilitySet::NONE)
    }

    /// Check whether `requested` capabilities fit within this policy.
    pub fn check(&self, requested: CapabilitySet) -> Result<(), CapabilityViolation> {
        enforce(requested, self.allowed)
    }
}

// ---------------------------------------------------------------------------
// Enforcement
// ---------------------------------------------------------------------------

/// Enforce that every capability in `requested` is present in `allowed`.
///
/// Returns `Ok(())` when enforcement passes, or `Err(CapabilityViolation)`
/// listing the denied capabilities.
pub fn enforce(
    requested: CapabilitySet,
    allowed: CapabilitySet,
) -> Result<(), CapabilityViolation> {
    let denied = requested.difference(allowed);
    if denied.is_empty() {
        Ok(())
    } else {
        let labels: Vec<&str> = denied.iter().map(|c| c.label()).collect();
        Err(CapabilityViolation {
            denied,
            message: format!("denied capabilities: {}", labels.join(", ")),
        })
    }
}

/// Convenience wrapper that maps a [`CapabilityViolation`] to
/// [`SecurityError::CapabilityDenied`].
pub fn enforce_or_error(requested: CapabilitySet, allowed: CapabilitySet) -> crate::Result<()> {
    enforce(requested, allowed).map_err(|v| SecurityError::CapabilityDenied {
        capability: v.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Individual capabilities ---

    #[test]
    fn test_single_capability_contains() {
        let set = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        assert!(set.contains(Capability::READ_TELEMETRY));
        assert!(!set.contains(Capability::WRITE_CONTROLS));
    }

    #[test]
    fn test_each_capability_individually() {
        for (cap, _) in Capability::ALL {
            let set = CapabilitySet::from_single(*cap);
            assert!(set.contains(*cap));
            assert!(!set.is_empty());
        }
    }

    #[test]
    fn test_capability_labels() {
        assert_eq!(Capability::READ_TELEMETRY.label(), "ReadTelemetry");
        assert_eq!(Capability::FILE_IO.label(), "FileIO");
    }

    // --- Set operations ---

    #[test]
    fn test_empty_set() {
        let set = CapabilitySet::NONE;
        assert!(set.is_empty());
        assert_eq!(set.iter().count(), 0);
    }

    #[test]
    fn test_union() {
        let a = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        let b = CapabilitySet::from_single(Capability::WRITE_CONTROLS);
        let u = a.union(b);
        assert!(u.contains(Capability::READ_TELEMETRY));
        assert!(u.contains(Capability::WRITE_CONTROLS));
        assert!(!u.contains(Capability::FILE_IO));
    }

    #[test]
    fn test_bitor_operator() {
        let a = CapabilitySet::from_single(Capability::NETWORK_IO);
        let b = CapabilitySet::from_single(Capability::FILE_IO);
        let c = a | b;
        assert!(c.contains(Capability::NETWORK_IO));
        assert!(c.contains(Capability::FILE_IO));
    }

    #[test]
    fn test_intersection() {
        let a = CapabilitySet::from_single(Capability::READ_TELEMETRY)
            .union(CapabilitySet::from_single(Capability::WRITE_CONTROLS));
        let b = CapabilitySet::from_single(Capability::WRITE_CONTROLS)
            .union(CapabilitySet::from_single(Capability::FILE_IO));
        let i = a.intersection(b);
        assert!(i.contains(Capability::WRITE_CONTROLS));
        assert!(!i.contains(Capability::READ_TELEMETRY));
        assert!(!i.contains(Capability::FILE_IO));
    }

    #[test]
    fn test_difference() {
        let all = CapabilitySet::from_single(Capability::READ_TELEMETRY)
            .union(CapabilitySet::from_single(Capability::WRITE_CONTROLS));
        let subset = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        let diff = all.difference(subset);
        assert!(!diff.contains(Capability::READ_TELEMETRY));
        assert!(diff.contains(Capability::WRITE_CONTROLS));
    }

    #[test]
    fn test_is_subset_of() {
        let small = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        let big = small.union(CapabilitySet::from_single(Capability::WRITE_CONTROLS));
        assert!(small.is_subset_of(big));
        assert!(!big.is_subset_of(small));
        assert!(small.is_subset_of(small));
    }

    #[test]
    fn test_from_bits_roundtrip() {
        let original = CapabilitySet::from_single(Capability::ACCESS_HID)
            .union(CapabilitySet::from_single(Capability::NETWORK_IO));
        let restored = CapabilitySet::from_bits(original.bits());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_display_empty() {
        assert_eq!(format!("{}", CapabilitySet::NONE), "(none)");
    }

    #[test]
    fn test_display_multiple() {
        let set = CapabilitySet::from_single(Capability::READ_TELEMETRY)
            .union(CapabilitySet::from_single(Capability::FILE_IO));
        let s = format!("{set}");
        assert!(s.contains("ReadTelemetry"));
        assert!(s.contains("FileIO"));
    }

    // --- Enforcement ---

    #[test]
    fn test_enforce_all_allowed() {
        let allowed = CapabilitySet::from_single(Capability::READ_TELEMETRY)
            .union(CapabilitySet::from_single(Capability::WRITE_CONTROLS));
        let requested = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        assert!(enforce(requested, allowed).is_ok());
    }

    #[test]
    fn test_enforce_exact_match() {
        let caps = CapabilitySet::from_single(Capability::ACCESS_HID);
        assert!(enforce(caps, caps).is_ok());
    }

    #[test]
    fn test_enforce_empty_request_always_ok() {
        assert!(enforce(CapabilitySet::NONE, CapabilitySet::NONE).is_ok());
    }

    #[test]
    fn test_enforce_violation_single() {
        let allowed = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        let requested = CapabilitySet::from_single(Capability::FILE_IO);
        let err = enforce(requested, allowed).unwrap_err();
        assert!(err.denied.contains(Capability::FILE_IO));
        assert!(err.message.contains("FileIO"));
    }

    #[test]
    fn test_enforce_violation_multiple() {
        let allowed = CapabilitySet::from_single(Capability::READ_TELEMETRY);
        let requested = CapabilitySet::from_single(Capability::FILE_IO)
            .union(CapabilitySet::from_single(Capability::NETWORK_IO));
        let err = enforce(requested, allowed).unwrap_err();
        assert!(err.denied.contains(Capability::FILE_IO));
        assert!(err.denied.contains(Capability::NETWORK_IO));
    }

    #[test]
    fn test_enforce_or_error() {
        let allowed = CapabilitySet::NONE;
        let requested = CapabilitySet::from_single(Capability::WRITE_CONTROLS);
        let err = enforce_or_error(requested, allowed).unwrap_err();
        assert!(format!("{err}").contains("WriteControls"));
    }

    // --- SandboxPolicy ---

    #[test]
    fn test_sandbox_policy_check_ok() {
        let policy = SandboxPolicy::new(
            "test",
            CapabilitySet::from_single(Capability::READ_TELEMETRY),
        );
        assert!(
            policy
                .check(CapabilitySet::from_single(Capability::READ_TELEMETRY))
                .is_ok()
        );
    }

    #[test]
    fn test_sandbox_policy_deny_all() {
        let policy = SandboxPolicy::deny_all("locked");
        let err = policy
            .check(CapabilitySet::from_single(Capability::FILE_IO))
            .unwrap_err();
        assert!(err.denied.contains(Capability::FILE_IO));
    }

    #[test]
    fn test_sandbox_policy_label() {
        let policy = SandboxPolicy::new("my-plugin", CapabilitySet::NONE);
        assert_eq!(policy.label, "my-plugin");
    }

    // --- Unknown capability bits ---

    #[test]
    fn test_from_bits_strips_unknown_bits() {
        let unknown_bit = 1 << 16;
        let set = CapabilitySet::from_bits(Capability::READ_TELEMETRY.0 | unknown_bit);
        assert!(set.contains(Capability::READ_TELEMETRY));
        assert_eq!(set.bits(), Capability::READ_TELEMETRY.0);
    }

    #[test]
    fn test_from_bits_strict_rejects_unknown_bits() {
        let unknown_bit = 1 << 16;
        let result = CapabilitySet::from_bits_strict(Capability::READ_TELEMETRY.0 | unknown_bit);
        let err = result.unwrap_err();
        assert_eq!(err.0, unknown_bit);
    }

    #[test]
    fn test_from_bits_strict_accepts_known_bits() {
        let bits = Capability::READ_TELEMETRY.0 | Capability::FILE_IO.0;
        let set = CapabilitySet::from_bits_strict(bits).unwrap();
        assert!(set.contains(Capability::READ_TELEMETRY));
        assert!(set.contains(Capability::FILE_IO));
        assert_eq!(set.bits(), bits);
    }
}
