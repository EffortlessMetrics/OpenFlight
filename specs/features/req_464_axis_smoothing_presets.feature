@REQ-464 @product
Feature: Axis Smoothing Preset System — Named Smoothing Presets  @AC-464.1
  Scenario: Smoothing presets include the standard named options
    Given the axis smoothing configuration
    When the available preset names are queried
    Then the presets SHALL include default, minimal, medium, aggressive, and custom  @AC-464.2
  Scenario: Selecting a preset applies its filter parameters automatically
    Given a virtual axis configured with the "aggressive" smoothing preset
    When axis values are processed
    Then the filter parameters SHALL match the aggressive preset definition without manual override  @AC-464.3
  Scenario: Custom preset allows full parameter override
    Given a virtual axis with smoothing preset set to "custom"
    When the user provides explicit alpha, window, and cutoff parameters
    Then those parameters SHALL be applied directly without being overridden by preset defaults  @AC-464.4
  Scenario: Active preset name is reported in axis diagnostics
    Given a virtual axis configured with the "medium" smoothing preset
    When the axis diagnostic data is retrieved
    Then the diagnostic output SHALL include the active preset name "medium"
