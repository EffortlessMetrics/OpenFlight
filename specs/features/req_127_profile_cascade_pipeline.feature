@REQ-127 @product
Feature: Profile cascade full pipeline

  @AC-127.1
  Scenario: Global profile applies defaults to all axes
    Given a global profile with deadzone 0.05 and expo 0.0 for all axes
    And no simulator, aircraft, or phase-of-flight profile is active
    When an axis value is processed
    Then the deadzone applied SHALL be 0.05
    And the expo applied SHALL be 0.0

  @AC-127.2
  Scenario: Simulator profile overrides global deadzone
    Given a global profile with deadzone 0.05
    And an active simulator profile with deadzone 0.10
    When an axis value is processed
    Then the deadzone applied SHALL be 0.10 from the simulator profile

  @AC-127.3
  Scenario: Aircraft profile overrides simulator expo
    Given a simulator profile with expo 0.2
    And an active aircraft profile with expo 0.5
    When an axis value is processed
    Then the expo applied SHALL be 0.5 from the aircraft profile

  @AC-127.4
  Scenario: Phase-of-flight approach profile overrides aircraft settings
    Given an aircraft profile with deadzone 0.08
    And an active approach phase-of-flight profile with deadzone 0.03
    When an axis value is processed during the approach phase
    Then the deadzone applied SHALL be 0.03 from the phase-of-flight profile

  @AC-127.5
  Scenario: More specific setting always wins in cascade
    Given a four-level cascade with global < simulator < aircraft < phase-of-flight
    When all four levels specify different values for the same axis setting
    Then the phase-of-flight value SHALL take precedence
    And global, simulator, and aircraft values SHALL be overridden

  @AC-127.6
  Scenario: Phase-of-flight profile reverts when phase changes
    Given an active approach phase-of-flight profile with deadzone 0.03
    When the phase changes from approach to cruise
    Then the approach phase-of-flight profile SHALL be deactivated
    And the aircraft profile deadzone SHALL be restored

  @AC-127.7
  Scenario: Profile swap is atomic with no partial state visible
    Given a running RT spine processing axis inputs
    When a new compiled profile is atomically swapped in at a tick boundary
    Then no tick SHALL observe a mixture of old and new profile settings
    And the swap SHALL complete within one tick period

  @AC-127.8
  Scenario: Multi-simulator switching does not corrupt another simulator's profile
    Given active profiles for both MSFS and X-Plane
    When the active simulator switches from MSFS to X-Plane
    Then the MSFS profile settings SHALL remain unchanged in memory
    And the X-Plane profile settings SHALL be applied correctly
