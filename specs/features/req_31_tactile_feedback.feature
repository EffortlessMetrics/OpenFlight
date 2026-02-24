@REQ-31
Feature: Tactile feedback effects and SimShaker bridge

  @AC-31.1
  Scenario: Effect intensity is validated on creation
    Given a tactile effect with intensity outside the valid range
    When the effect is created
    Then creation SHALL fail with an intensity validation error

  @AC-31.1
  Scenario: Expired effects are discarded
    Given a tactile effect with a short expiry duration
    When the effect expires
    Then the effect SHALL be discarded from the active effects queue

  @AC-31.2
  Scenario: Touchdown event triggers tactile effect
    Given a tactile bridge monitoring landing events
    When a touchdown is detected in the flight telemetry
    Then a touchdown tactile effect SHALL be emitted

  @AC-31.2
  Scenario: Stall buffet event triggers tactile effect
    Given a tactile bridge monitoring aerodynamic events
    When a stall buffet condition is detected
    Then a stall buffet tactile effect SHALL be emitted

  @AC-31.2
  Scenario: Ground roll event triggers tactile effect
    Given a tactile bridge monitoring ground operations
    When ground roll is detected
    Then a ground roll tactile effect SHALL be emitted

  @AC-31.3
  Scenario: SimShaker configuration is validated
    Given a SimShaker bridge with an invalid configuration
    When the bridge is initialized
    Then initialization SHALL fail with a configuration validation error

  @AC-31.3
  Scenario: SimShaker packets are created with correct channel values
    Given a SimShaker bridge with a valid configuration
    When a tactile effect is mapped to a channel
    Then the resulting packet SHALL contain the expected channel value
