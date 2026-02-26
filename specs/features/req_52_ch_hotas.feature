@REQ-52 @product
Feature: CH Products HOTAS axis presets and health monitoring

  @AC-52.1
  Scenario: Every CH Products device model has a recommended preset
    Given all six CH Products device models (Fighterstick, CombatStick, ProThrottle, ProPedals, EclipseYoke, FlightYoke)
    When recommended_preset is called for each model
    Then a ChAxisPreset SHALL be returned whose device field matches the requested model

  @AC-52.2
  Scenario: Preset deadzone is within valid range for all models
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then the deadzone field SHALL be within [0.0, 0.1] inclusive

  @AC-52.2
  Scenario: Proptest confirms deadzone validity across all model indices
    Given a model index selected in 0..6
    When recommended_preset is called for that model
    Then the deadzone SHALL always be in [0.0, 0.1]

  @AC-52.3
  Scenario: Preset expo is within valid range for all models
    Given all six CH Products device models
    When recommended_preset is called for each model
    Then the expo field SHALL be within [0.0, 0.5] inclusive

  @AC-52.3
  Scenario: Proptest confirms expo validity across all model indices
    Given a model index selected in 0..6
    When recommended_preset is called for that model
    Then the expo SHALL always be in [0.0, 0.5]

  @AC-52.4
  Scenario: Yoke models have throttle inversion enabled
    Given CH Products device models EclipseYoke and FlightYoke
    When recommended_preset is called for each
    Then invert_throttle SHALL be true

  @AC-52.4
  Scenario: Non-yoke models do not invert throttle
    Given CH Products device models Fighterstick, CombatStick, ProThrottle, and ProPedals
    When recommended_preset is called for each
    Then invert_throttle SHALL be false

  @AC-52.5
  Scenario: Health monitor initial status is Unknown
    Given a ChHealthMonitor created for the Fighterstick model
    When the status is queried before any update
    Then the status SHALL be Unknown

  @AC-52.5
  Scenario: Health monitor reflects Connected after update
    Given a ChHealthMonitor created for the ProThrottle model
    When update_status is called with Connected
    Then the status SHALL be Connected

  @AC-52.5
  Scenario: Health monitor reflects Disconnected after reconnect cycle
    Given a ChHealthMonitor that was previously Connected
    When update_status is called with Disconnected
    Then the status SHALL be Disconnected

  @AC-52.5
  Scenario: Health monitor preserves the tracked device model
    Given a ChHealthMonitor created for the EclipseYoke model
    When the model is queried
    Then model() SHALL return EclipseYoke

  @AC-52.5
  Scenario: All CH Products models can be individually monitored
    Given each of the six CH Products device models
    When a ChHealthMonitor is created and set to Connected for each
    Then status() SHALL return Connected for every model
