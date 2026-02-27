@REQ-182 @product
Feature: GoFlight module panels integrate with OpenFlight's panel engine

  @AC-182.1
  Scenario: GoFlight GF-46 COM/NAV radio panel detected via HID
    Given a GoFlight GF-46 COM/NAV radio panel is connected via USB
    When the panel engine enumerates HID devices
    Then the GF-46 SHALL be detected and its knobs and buttons registered with the panel engine

  @AC-182.2
  Scenario: GoFlight GF-45 autopilot panel inputs routed to panel engine
    Given a GoFlight GF-45 autopilot panel is connected
    When knobs or buttons on the GF-45 are operated
    Then their inputs SHALL be routed through the panel engine for profile-defined action dispatch

  @AC-182.3
  Scenario: GoFlight GF-LGT landing gear LEDs controlled via profile
    Given a GoFlight GF-LGT landing gear panel is connected with LED mappings defined in the profile
    When the profile dictates a specific LED state
    Then the GF-LGT LEDs SHALL reflect that state as commanded

  @AC-182.4
  Scenario: Multiple GoFlight modules on same USB hub work independently
    Given two or more GoFlight modules are connected through the same USB hub
    When each module receives separate inputs or LED commands
    Then each module SHALL operate independently without cross-talk or command misrouting

  @AC-182.5
  Scenario: GoFlight panel state survives sim pause and unpause
    Given a GoFlight panel is active and tracking simulator state
    When the simulator is paused and then unpaused
    Then the panel state SHALL remain consistent throughout the pause cycle without requiring re-initialisation

  @AC-182.6
  Scenario: GoFlight encoder steps mapped to integer sim variable deltas
    Given a profile that maps a GoFlight encoder to a simulator integer variable
    When the encoder is rotated by one or more detents
    Then the simulator variable SHALL change by the configured integer delta per encoder step
