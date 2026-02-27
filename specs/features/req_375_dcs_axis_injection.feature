@REQ-375 @product
Feature: DCS Axis Injection via LoSetCommand — Send Values to DCS

  @AC-375.1
  Scenario: Processed axis values are sent to DCS via LoSetCommand UDP
    Given an axis configured with a DCS LoSetCommand target
    When the axis produces a processed output value
    Then the value SHALL be transmitted to DCS via a LoSetCommand UDP message

  @AC-375.2
  Scenario: Axis-to-command mapping is configurable in the DCS export profile
    Given a DCS export profile specifying axis-to-LoSetCommand mappings
    When the profile is loaded
    Then each axis SHALL send its output to the configured LoSetCommand

  @AC-375.3
  Scenario: Injection rate does not exceed 50 Hz
    Given an axis configured for DCS UDP injection
    When the injection loop is active
    Then the injection rate SHALL NOT exceed 50 Hz to respect DCS export limitations

  @AC-375.4
  Scenario: DCS UDP injection coexists with DCS telemetry read
    Given both DCS telemetry reading and axis injection are active
    When both operate concurrently
    Then neither SHALL interfere with the other and both SHALL function correctly

  @AC-375.5
  Scenario: Injection errors are logged with command name and error code
    Given an axis configured for DCS UDP injection
    When a UDP send fails for a LoSetCommand
    Then the error SHALL be logged with the command name and the error code

  @AC-375.6
  Scenario: Integration test with mock DCS UDP socket verifies injected commands
    Given a mock DCS UDP socket and an axis configured for LoSetCommand injection
    When the axis processes a known input value
    Then the mock socket SHALL receive the expected LoSetCommand UDP packet
