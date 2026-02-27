@REQ-101
Feature: Profile cascade and aircraft auto-switch

  @AC-101.1
  Scenario: Global profile provides fallback values
    Given a session with a global profile loaded and no aircraft-specific override
    When an axis mapping is queried for the active aircraft
    Then the global profile value SHALL be returned as the fallback

  @AC-101.2
  Scenario: Aircraft-specific profile overrides global for same axis
    Given a session with both a global profile and an aircraft-specific profile
    When both profiles define a mapping for the same axis
    Then the aircraft profile value SHALL take precedence over the global value

  @AC-101.3
  Scenario: Phase-of-flight hysteresis prevents flapping on a single outlier frame
    Given a session manager with hysteresis configured
    When a single telemetry frame suggests a different phase of flight
    Then the current phase SHALL not change

  @AC-101.4
  Scenario: Failed profile load preserves the running profile
    Given an active session with a successfully loaded profile
    When a subsequent profile reload attempt fails
    Then the previously loaded profile SHALL remain active and unchanged

  @AC-101.5
  Scenario: Aircraft switch completes within 500ms
    Given a session manager monitoring aircraft identity
    When an aircraft-change event is received
    Then the new profile SHALL be active within 500 milliseconds
