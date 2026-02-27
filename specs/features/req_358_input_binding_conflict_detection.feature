@REQ-358 @product
Feature: Input Binding Conflict Detection  @AC-358.1
  Scenario: Two bindings targeting the same logical output are flagged as conflicts
    Given two profile entries both mapping to logical axis "pitch"
    When the profile is loaded
    Then the loader SHALL report a binding conflict for "pitch"  @AC-358.2
  Scenario: Conflicts are reported with source device, axis ID, and profile path
    Given a binding conflict exists
    When the conflict report is generated
    Then the report SHALL include source device name, axis or button ID, and the conflicting profile file path  @AC-358.3
  Scenario: Profile load fails in strict mode
    Given strict_conflicts is enabled in the profile
    When a conflicting binding is detected at load time
    Then profile loading SHALL fail with a descriptive error message  @AC-358.4
  Scenario: Conflict report is available via flightctl bindings check
    Given a profile with conflicting bindings is active
    When the user runs "flightctl bindings check"
    Then the CLI SHALL output the full conflict report and exit with a non-zero code  @AC-358.5
  Scenario: Shadowed bindings are warned but allowed
    Given two bindings from different priority layers target the same source axis
    When the profile is loaded without strict_conflicts
    Then a warning SHALL be logged and the higher-priority binding SHALL take effect  @AC-358.6
  Scenario: No false positives for non-conflicting binding sets
    Given a profile where all bindings target distinct logical outputs
    When the conflict checker runs
    Then no conflicts SHALL be reported
