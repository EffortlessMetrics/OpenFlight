@REQ-674
Feature: Axis Temperature Drift Correction
  @AC-674.1
  Scenario: Drift correction monitors center position over time
    Given the system is configured for REQ-674
    When the feature condition is met
    Then drift correction monitors center position over time

  @AC-674.2
  Scenario: Gradual drift is compensated without user intervention
    Given the system is configured for REQ-674
    When the feature condition is met
    Then gradual drift is compensated without user intervention

  @AC-674.3
  Scenario: Sudden large shifts trigger a recalibration advisory
    Given the system is configured for REQ-674
    When the feature condition is met
    Then sudden large shifts trigger a recalibration advisory

  @AC-674.4
  Scenario: Drift correction rate is configurable in service settings
    Given the system is configured for REQ-674
    When the feature condition is met
    Then drift correction rate is configurable in service settings
