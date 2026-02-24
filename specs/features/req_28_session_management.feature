@REQ-28
Feature: Session management and phase-of-flight determination

  @AC-28.1
  Scenario: Aircraft ID supports basic operations
    Given an AircraftId created from a title string
    When equality and display operations are performed
    Then the aircraft ID SHALL behave correctly for comparison and formatting

  @AC-28.2
  Scenario: Phase-of-flight is determined from telemetry
    Given a session manager receiving live flight telemetry
    When the telemetry indicates a cruise flight state
    Then the detected phase SHALL be Cruise
    And the phase string representation SHALL be human-readable

  @AC-28.3
  Scenario: Hysteresis prevents rapid phase oscillation
    Given a session manager with hysteresis band configured
    When telemetry oscillates near a phase boundary
    Then the session SHALL not oscillate between phases on consecutive frames

  @AC-28.3
  Scenario: Consecutive-frame requirement prevents single-frame phase flip
    Given a session manager requiring N consecutive frames to confirm a phase
    When a different phase appears for fewer than N frames
    Then the session SHALL retain the previous stable phase

  @AC-28.4
  Scenario: Profile hierarchy loads in precedence order
    Given simulator, aircraft, and phase-specific profiles
    When the session loads the profile hierarchy
    Then aircraft profile values SHALL override simulator profile values
    And phase profile values SHALL override aircraft profile values
