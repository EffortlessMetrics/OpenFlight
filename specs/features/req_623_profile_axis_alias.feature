Feature: Profile Axis Alias
  As a flight simulation enthusiast
  I want to define human-readable axis aliases in my profile
  So that profile rules are easier to read and maintain

  Background:
    Given the OpenFlight service is running

  Scenario: Axis aliases map human-readable names to physical axis IDs
    Given a profile with an axis alias mapping "pitch" to a physical axis ID
    When the profile is loaded
    Then the alias resolves to the correct physical axis

  Scenario: Aliases are usable in profile rules as axis references
    Given a profile with a defined axis alias
    When a profile rule references the alias name
    Then the rule applies to the aliased physical axis

  Scenario: Duplicate alias names produce a validation error
    Given a profile that defines the same alias name twice
    When the profile is validated
    Then a validation error is reported for the duplicate alias

  Scenario: Aliases are shown in axis diagnostics
    Given a profile with axis aliases is active
    When axis diagnostics are queried
    Then the alias names are shown alongside the physical axis IDs
