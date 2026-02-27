@REQ-481 @product
Feature: Axis Statistics Collection — Runtime Per-Axis Statistics  @AC-481.1
  Scenario: Statistics include min, max, mean, and standard deviation of values
    Given an axis has been processing input values for at least one statistics window
    When statistics are queried for that axis
    Then the response SHALL include min, max, mean, and standard deviation of observed values  @AC-481.2
  Scenario: Statistics are computed over configurable sliding window
    Given an axis statistics window configured to 10 seconds
    When statistics are queried after 15 seconds of activity
    Then the statistics SHALL reflect only the most recent 10 seconds of values  @AC-481.3
  Scenario: Statistics are accessible via IPC without impacting RT performance
    Given the RT spine is running at 250Hz
    When statistics are requested via IPC during active processing
    Then the statistics query SHALL complete without introducing measurable jitter on the RT spine  @AC-481.4
  Scenario: Statistics reset is triggerable via CLI command
    Given an axis with accumulated statistics
    When `flightctl axis stats reset <axis_id>` is executed
    Then the statistics for that axis SHALL be cleared and resume accumulation from zero
