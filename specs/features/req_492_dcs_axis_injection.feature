@REQ-492 @product
Feature: DCS Axis Injection — Processed Axis Value Injection via Export API  @AC-492.1
  Scenario: Adapter sends axis values to DCS via export socket commands
    Given the DCS export adapter is connected to a running DCS instance
    When processed axis values are available
    Then the adapter SHALL transmit the values via the export socket  @AC-492.2
  Scenario: Injection supports rudder, throttle, aileron, elevator axes
    Given the DCS axis injection is enabled
    When axis values for rudder, throttle, aileron, or elevator are processed
    Then each axis SHALL be injected to DCS via the corresponding export command  @AC-492.3
  Scenario: Injection is gated by enable_injection config flag
    Given the service config has enable_injection set to false
    When processed axis values are available
    Then the adapter SHALL NOT send injection commands to DCS  @AC-492.4
  Scenario: Failed injections are counted and logged per axis
    Given axis injection is enabled and an injection command fails
    When the failure occurs
    Then the failure SHALL be counted in per-axis metrics and a log entry SHALL be emitted
