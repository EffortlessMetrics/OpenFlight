@REQ-513 @product
Feature: Axis Sensitivity Curve Presets

  @AC-513.1 @AC-513.2
  Scenario: Selecting a named preset replaces curve config atomically
    Given the axis pipeline is running at 250 Hz
    When the user selects the "aggressive" sensitivity preset
    Then the axis curve config SHALL be replaced atomically on the next tick
    And no intermediate values SHALL be processed with a partial config

  @AC-513.3
  Scenario: Custom curves are preserved as user-defined presets
    Given a user has configured a custom axis curve
    When the user saves it as a named preset
    Then the preset SHALL be stored in the profile and available for future selection

  @AC-513.4
  Scenario: Active preset is shown in flightctl axis status output
    Given a sensitivity preset is active for an axis
    When the user runs flightctl axis status
    Then the output SHALL include the name of the active preset for each axis
