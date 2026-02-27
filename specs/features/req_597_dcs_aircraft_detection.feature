Feature: DCS Aircraft Module Detection
  As a flight simulation enthusiast
  I want the DCS adapter to detect the loaded aircraft module
  So that profiles can be automatically selected for the active aircraft

  Background:
    Given the DCS adapter is connected and receiving telemetry

  Scenario: Aircraft module name is extracted from DCS telemetry
    When DCS telemetry contains aircraft module information
    Then the adapter extracts and stores the module name

  Scenario: Aircraft detection event is published on flight-bus
    When the detected aircraft module changes
    Then an aircraft detection event with the module name is published on flight-bus

  Scenario: Profile auto-select runs on aircraft module change
    Given a profile is associated with the DCS aircraft module "F-16C_50"
    When the DCS adapter detects aircraft module "F-16C_50"
    Then the associated profile is automatically selected

  Scenario: Module name is available in adapter diagnostics
    Given the DCS adapter has detected an aircraft module
    When the adapter diagnostics are queried
    Then the current aircraft module name is included in the diagnostics output
