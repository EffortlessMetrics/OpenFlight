@REQ-340 @product
Feature: Per-Axis Logging  @AC-340.1
  Scenario: Logging can be enabled independently per axis
    Given a profile with three mapped axes
    When logging is enabled for only one axis
    Then only that axis SHALL emit log entries; the other two SHALL remain silent  @AC-340.2
  Scenario: Log level is configurable per axis
    Given an axis with per-axis logging configured at "trace" level
    When the axis processes a value
    Then the log entry SHALL be emitted at trace level  @AC-340.3
  Scenario: Axis log entry includes all processing stages
    Given an axis with per-axis logging enabled
    When the axis processes an input value
    Then each log entry SHALL contain raw, post-deadzone, post-curve, and final output values  @AC-340.4
  Scenario: Axis logging can be toggled via CLI without service restart
    Given the service is running with axis logging disabled for "rudder"
    When the user runs "flightctl axis log enable rudder"
    Then log entries for the "rudder" axis SHALL begin appearing without restarting the service  @AC-340.5
  Scenario: Log output is structured JSON
    Given an axis with per-axis logging enabled
    When a log entry is written
    Then the entry SHALL be valid JSON that can be parsed by a machine  @AC-340.6
  Scenario: High-frequency logging requires explicit opt-in
    Given a logging rate greater than 10Hz is requested for an axis
    When the configuration is applied without the explicit high-frequency opt-in flag
    Then the service SHALL reject the configuration with an error indicating that the opt-in flag is required
