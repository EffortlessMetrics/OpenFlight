Feature: MSFS Simvar Units Conversion
  As a flight simulation enthusiast
  I want the SimConnect adapter to handle MSFS unit conversions
  So that all published variable values are in consistent SI units

  Background:
    Given the OpenFlight service is running and connected to MSFS via SimConnect

  Scenario: All SimVar values are converted to SI units before publishing
    Given MSFS reports "PLANE_ALTITUDE" in feet
    When the SimConnect adapter receives the value
    Then the published value is converted to metres before being placed on the flight-bus

  Scenario: Unit conversion is documented per variable in code comments
    When the SimConnect adapter source code is reviewed
    Then each SimVar subscription includes a comment stating the source unit and target SI unit

  Scenario: Unit conversion is tested with boundary values
    Given unit conversion tests are defined for "PLANE_ALTITUDE"
    When the tests are run with boundary values including 0, max altitude, and negative altitudes
    Then all converted values match the expected SI equivalents

  Scenario: Converted values match expected MSFS variable definitions
    Given the MSFS SDK variable definitions are available
    When the adapter subscribes to a variable
    Then the unit used in the SimConnect request matches the MSFS SDK definition for that variable
