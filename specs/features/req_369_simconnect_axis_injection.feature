@REQ-369 @product
Feature: SimConnect Axis Injection — Send Axis Outputs Back to MSFS

  @AC-369.1
  Scenario: Processed axis values can be sent back to MSFS
    Given an axis with SimConnect injection enabled
    When the axis produces a processed output value
    Then the value SHALL be transmitted to MSFS via the SimConnect interface

  @AC-369.2
  Scenario: Injection uses SIMCONNECT_INPUT_EVENT or axis-specific events
    Given an axis configured for SimConnect injection
    When the injection mechanism sends a value
    Then it SHALL use SIMCONNECT_INPUT_EVENT or the appropriate axis-specific SimConnect event

  @AC-369.3
  Scenario: Injection can be enabled or disabled per axis
    Given a profile with multiple axes
    When injection is disabled on a specific axis
    Then that axis SHALL NOT transmit values to MSFS via SimConnect

  @AC-369.4
  Scenario: Injection rate does not exceed 250 Hz
    Given an axis with SimConnect injection enabled
    When the RT spine processes ticks at 250 Hz
    Then the injection rate SHALL NOT exceed 250 Hz

  @AC-369.5
  Scenario: Injection errors are counted and exposed via metrics
    Given an axis with SimConnect injection enabled
    When a SimConnect injection call fails
    Then the failure SHALL be counted and the count SHALL be exposed via the metrics endpoint

  @AC-369.6
  Scenario: Integration test with mock SimConnect verifies injected values
    Given a mock SimConnect interface and an axis with injection enabled
    When the axis processes a known input value
    Then the mock interface SHALL record the expected injected value
