@REQ-297 @product
Feature: Telemetry Normalization Pipeline  @AC-297.1
  Scenario: Raw telemetry values from all simulators are normalized to a common schema
    Given telemetry data arrives from MSFS, X-Plane, and DCS adapters
    When the normalization pipeline processes each telemetry frame
    Then all values SHALL conform to the common internal telemetry schema regardless of source simulator  @AC-297.2
  Scenario: Altitude is always in meters in the internal representation
    Given a simulator reports altitude in feet
    When the normalization pipeline processes the altitude value
    Then the internal representation SHALL store altitude in meters  @AC-297.3
  Scenario: Speed is always in m/s internally with configurable display units
    Given a simulator reports airspeed in knots
    When the normalization pipeline processes the speed value
    Then the internal representation SHALL store speed in m/s and display units SHALL be configurable separately  @AC-297.4
  Scenario: Normalized telemetry is available to all consumers without conversion
    Given the normalization pipeline has processed a telemetry frame
    When any internal consumer reads the telemetry
    Then it SHALL receive values in the normalized schema without performing additional unit conversion  @AC-297.5
  Scenario: Normalization failures produce a logged warning not a crash
    Given the normalization pipeline receives a malformed telemetry frame
    When normalization fails for one or more fields
    Then the service SHALL log a warning and continue processing subsequent frames without crashing  @AC-297.6
  Scenario: Unit conversion factors are externally configurable
    Given a unit conversion configuration file exists
    When the service loads at startup
    Then conversion factors SHALL be read from the configuration file and not be hardcoded in the binary
