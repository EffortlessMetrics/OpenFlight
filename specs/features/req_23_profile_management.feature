@REQ-23
Feature: Profile management and hierarchical merging

  @AC-23.1
  Scenario: Profile validation rejects invalid configurations
    Given a profile with an invalid axis configuration
    When the profile is validated
    Then validation SHALL return an error identifying the invalid field

  @AC-23.1
  Scenario: Profile validation accepts valid configurations
    Given a well-formed profile with valid axis and button mappings
    When the profile is validated
    Then validation SHALL succeed with no errors

  @AC-23.2
  Scenario: Capability enforcement rejects unsupported features
    Given a profile requiring force-feedback on a non-FFB device
    When capability enforcement is applied
    Then the enforcement SHALL report a capability violation

  @AC-23.3
  Scenario: Profile canonicalization produces stable hashes
    Given two profiles with identical logical configuration but different field order
    When both are canonicalized
    Then their effective hashes SHALL be equal

  @AC-23.4
  Scenario: merge_with applies more-specific profile overrides
    Given a global profile and an aircraft-specific override profile
    When merge_with is called with the override as the more-specific source
    Then the resulting profile SHALL use override values for fields present in the override
    And retain global values for fields absent from the override

  @AC-23.5
  Scenario: Profile auto-loads on aircraft detection
    Given the service is running with a global profile and an aircraft-specific profile for "F-16C"
    When the aircraft detector emits an AircraftDetected event for "F-16C"
    Then the service SHALL load the aircraft-specific profile within 500ms
    And the active profile SHALL reflect the F-16C-specific axis configuration

  @AC-23.5
  Scenario: Fallback profile used when aircraft-specific profile is unavailable
    Given the service is running with a global profile only
    When the aircraft detector emits an AircraftDetected event for an aircraft with no specific profile
    Then the service SHALL continue using the global profile
    And no error SHALL be raised for the missing aircraft profile

  @AC-23.5
  Scenario: Profile priority resolution follows global-to-aircraft cascade
    Given profiles exist at global, simulator, and aircraft levels
    When the profiles are merged for an active aircraft
    Then aircraft-level values SHALL override simulator-level values
    And simulator-level values SHALL override global-level values
    And all unoverridden global values SHALL remain active

  @AC-23.6
  Scenario: Auto-switch handles adapter initialization failure gracefully
    Given the service attempts to start the MSFS SimConnect adapter
    When SimConnect is unavailable at startup
    Then the auto-switch service SHALL log the failure reason
    And SHALL continue running in degraded mode using the global profile
    And SHALL not crash or enter an unrecoverable state

  @AC-23.6
  Scenario: Auto-switch recovers from partial telemetry loss
    Given an active sim session where aircraft telemetry is flowing
    When the telemetry stream produces no updates for more than 5 seconds
    Then the bus snapshot SHALL be marked as stale
    And any force-feedback consumers SHALL stop applying forces until telemetry resumes
    And the service SHALL emit a diagnostic event describing the stale data condition
