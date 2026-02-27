@REQ-370 @product
Feature: X-Plane Dataref Write — Send Processed Values to X-Plane

  @AC-370.1
  Scenario: Processed axis values are written to X-Plane datarefs via UDP
    Given an axis configured with a target X-Plane dataref
    When the axis produces a processed output value
    Then the value SHALL be written to X-Plane via the UDP dataref write mechanism

  @AC-370.2
  Scenario: Target dataref paths are configurable per axis in the profile
    Given a profile with multiple axes each targeting different datarefs
    When the profile is loaded
    Then each axis SHALL write to its configured dataref path

  @AC-370.3
  Scenario: Write rate is configurable with default matching telemetry read rate
    Given an axis configured for X-Plane dataref write
    When no explicit write rate is configured
    Then the write rate SHALL default to match the telemetry read rate

  @AC-370.4
  Scenario: Unknown datarefs are logged once and silently ignored
    Given an axis configured with a dataref path that does not exist in X-Plane
    When the first write attempt is made
    Then a warning SHALL be logged once and subsequent writes SHALL be silently ignored

  @AC-370.5
  Scenario: Dataref write errors are counted per dataref
    Given an axis configured for X-Plane dataref write
    When a write error occurs for a specific dataref
    Then the error count for that dataref SHALL be incremented in metrics

  @AC-370.6
  Scenario: Integration test with mock X-Plane socket verifies written values
    Given a mock X-Plane UDP socket and an axis configured for dataref write
    When the axis processes a known input value
    Then the mock socket SHALL receive the expected dataref write packet
