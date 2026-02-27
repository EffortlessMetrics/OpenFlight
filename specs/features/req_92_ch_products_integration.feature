@REQ-92 @product
Feature: CH Products Axis Presets and Health Monitor Integration Tests

  Background:
    Given the flight-hotas-ch crate with all six CH Products device models
    And the models are Fighterstick, CombatStick, ProThrottle, ProPedals, EclipseYoke, FlightYoke

  @AC-92.1
  Scenario: All CH Products device models have a recommended preset whose device field matches
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then the returned ChAxisPreset SHALL have its device field equal to the requested model

  @AC-92.2
  Scenario: Deadzone preset value is within valid range [0.0, 0.5] for every model
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then the deadzone field SHALL be within [0.0, 0.5] inclusive

  @AC-92.2
  Scenario: Expo preset value is within valid range [0.0, 1.0] for every model
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then the expo field SHALL be within [0.0, 1.0] inclusive

  @AC-92.2
  Scenario: Deadzone and expo are finite for every model
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then deadzone SHALL be finite
    And expo SHALL be finite

  @AC-92.3
  Scenario: Yoke models have invert_throttle enabled
    Given CH Products models EclipseYoke and FlightYoke
    When recommended_preset is called for each
    Then invert_throttle SHALL be true

  @AC-92.3
  Scenario: Non-yoke models do not have invert_throttle enabled
    Given CH Products models Fighterstick, CombatStick, ProThrottle, and ProPedals
    When recommended_preset is called for each
    Then invert_throttle SHALL be false

  @AC-92.4
  Scenario: Health monitor initial status is Unknown for every model
    Given a ChHealthMonitor newly created for each CH Products model
    When status is queried before any update
    Then status() SHALL return Unknown for every model

  @AC-92.4
  Scenario: Health monitor transitions to Connected after update
    Given a ChHealthMonitor for the Fighterstick model
    When update_status is called with Connected
    Then status() SHALL return Connected

  @AC-92.4
  Scenario: Health monitor transitions to Disconnected after disconnect
    Given a ChHealthMonitor for the ProThrottle model that was previously Connected
    When update_status is called with Disconnected
    Then status() SHALL return Disconnected

  @AC-92.4
  Scenario: Health monitor re-connects after a disconnect cycle
    Given a ChHealthMonitor for the ProPedals model that was Disconnected
    When update_status is called with Connected
    Then status() SHALL return Connected

  @AC-92.4
  Scenario: Health monitor preserves the tracked device model
    Given a ChHealthMonitor newly created for each of the six CH Products models
    When model() is queried
    Then model() SHALL return the model that was passed to the constructor

  @AC-92.4
  Scenario: All CH Products models complete a full Unknown → Connected → Disconnected → Connected cycle
    Given a ChHealthMonitor for each of the six CH Products models
    When the full state cycle is executed (Connected then Disconnected then Connected)
    Then each monitor SHALL end in the Connected state
