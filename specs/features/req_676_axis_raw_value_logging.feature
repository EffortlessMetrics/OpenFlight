@REQ-676
Feature: Axis Raw Value Logging
  @AC-676.1
  Scenario: Raw axis values can be logged to file for diagnostics
    Given the system is configured for REQ-676
    When the feature condition is met
    Then raw axis values can be logged to file for diagnostics

  @AC-676.2
  Scenario: Log format includes timestamp and device identifier
    Given the system is configured for REQ-676
    When the feature condition is met
    Then log format includes timestamp and device identifier

  @AC-676.3
  Scenario: Logging can be enabled per-axis without restarting service
    Given the system is configured for REQ-676
    When the feature condition is met
    Then logging can be enabled per-axis without restarting service

  @AC-676.4
  Scenario: Log files are automatically rotated at configurable size
    Given the system is configured for REQ-676
    When the feature condition is met
    Then log files are automatically rotated at configurable size
