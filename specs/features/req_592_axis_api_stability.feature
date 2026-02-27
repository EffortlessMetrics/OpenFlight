Feature: Axis Engine Rust API Stability
  As a flight simulation enthusiast
  I want the flight-axis public API to be semver stable
  So that downstream consumers can rely on a stable interface

  Background:
    Given the flight-axis crate is published with a stable version

  Scenario: Breaking API changes require major version bump
    Given a change removes or modifies a public type in flight-axis
    When the crate version is updated
    Then the major version number is incremented

  Scenario: Semver-exempt items are marked with unstable feature flag
    Given an experimental API is added to flight-axis
    When the item is gated behind the "unstable" feature flag
    Then consumers without the "unstable" feature cannot access the item

  Scenario: API surface is documented in crate-level rustdoc
    When the flight-axis crate documentation is generated with cargo doc
    Then all public types, traits, and functions have rustdoc comments

  Scenario: Semver compatibility is checked in CI via cargo-semver-checks
    When a pull request modifies flight-axis
    Then cargo-semver-checks runs in CI and reports any semver violations
