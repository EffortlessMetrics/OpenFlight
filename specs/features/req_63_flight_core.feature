@REQ-63
Feature: Core types, profile management, and aircraft detection

  @AC-63.1
  Scenario: Profile round-trips through JSON without data loss
    Given a profile with axes, curve points, and detent zones
    When the profile is serialized to JSON and deserialized back
    Then all fields SHALL be preserved and the effective hash SHALL be unchanged

  @AC-63.1
  Scenario: Profile round-trips through YAML and TOML
    Given a fully-populated profile with sim and aircraft fields
    When the profile is serialized to YAML and to TOML then deserialized
    Then all fields SHALL be preserved in both formats

  @AC-63.2
  Scenario: merge_with replaces override fields from the base
    Given a base profile with expo 0.3 on axis "pitch"
    When merged with an override that sets expo 0.7
    Then the merged profile SHALL have expo 0.7 on "pitch"

  @AC-63.2
  Scenario: merge_with does not clobber absent override fields
    Given a base profile with slew_rate set on axis "roll"
    When merged with an override that has no slew_rate for "roll"
    Then the merged profile SHALL retain the original slew_rate

  @AC-63.3
  Scenario: Validation rejects invalid schema versions and out-of-range values
    Given profiles with unknown schema version, negative deadzone, or expo above 1.0
    When each profile is validated
    Then each SHALL return a validation error describing the violation

  @AC-63.4
  Scenario: Capability modes enforce tighter limits
    Given a profile in kid mode with expo above the kid-mode maximum
    When the profile is validated under kid mode
    Then validation SHALL fail with an expo-too-high error

  @AC-63.5
  Scenario: effective_hash is stable and discriminates distinct profiles
    Given a profile P1
    When effective_hash is called twice on P1 and once on a distinct profile P2
    Then both calls on P1 SHALL return the same value
    And the value for P2 SHALL differ from P1
