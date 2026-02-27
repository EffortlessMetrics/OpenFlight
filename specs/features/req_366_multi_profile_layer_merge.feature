@REQ-366 @product
Feature: Multi-Profile Layer Merge  @AC-366.1
  Scenario: Global profile provides baseline values for all unspecified fields
    Given a global profile with a full set of default axis settings
    When no sim, aircraft, or phase profile overrides a field
    Then the compiled profile SHALL use the global profile value for that field  @AC-366.2
  Scenario: Sim overrides global, aircraft overrides sim, phase overrides aircraft
    Given global, sim, aircraft, and phase profiles each specifying the same field with different values
    When the profiles are merged
    Then the compiled profile SHALL use the phase-level value for that field  @AC-366.3
  Scenario: Merge produces a single flat compiled profile snapshot
    Given all four profile layers are present and loaded
    When the merge operation completes
    Then the result SHALL be a single flat compiled profile with no layer references  @AC-366.4
  Scenario: Layer priority is phase then aircraft then sim then global
    Given conflicting values in multiple profile layers
    When the profile merge is computed
    Then the priority order SHALL be phase over aircraft over sim over global  @AC-366.5
  Scenario: Missing intermediate layers are skipped without error
    Given only a global profile and a phase profile are present with no sim or aircraft layer
    When the profile merge is performed
    Then it SHALL complete successfully using the available layers  @AC-366.6
  Scenario: Profile merge is deterministic given the same inputs
    Given a fixed set of global, sim, aircraft, and phase profiles
    When the merge operation is run multiple times with the same inputs
    Then each run SHALL produce an identical compiled profile snapshot
