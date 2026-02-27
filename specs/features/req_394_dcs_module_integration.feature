@REQ-394 @product
Feature: DCS Module Integration — Load Aircraft-Specific Axis Mapping from Module Files

  @AC-394.1
  Scenario: DCS module directory is scanned for aircraft modules at startup
    Given a DCS module directory containing module files
    When the service starts
    Then all supported aircraft modules SHALL be discovered and loaded

  @AC-394.2
  Scenario: Per-aircraft axis maps are loaded from module files
    Given a DCS module file for a specific aircraft
    When the module is loaded
    Then stick throw and throttle range axis maps SHALL be available for that aircraft

  @AC-394.3
  Scenario: Module files use TOML format with axis range and quirk fields
    Given a DCS module file on disk
    When it is parsed
    Then it SHALL be valid TOML containing axis range and quirk fields

  @AC-394.4
  Scenario: Unknown DCS aircraft fall back to generic axis mapping
    Given an active DCS session with an unrecognised aircraft
    When axis mapping is requested
    Then the generic axis mapping SHALL be applied as a fallback

  @AC-394.5
  Scenario: Module scan is non-blocking and runs on the startup thread
    Given the service startup sequence
    When the DCS module scan executes
    Then it SHALL run on the startup thread and SHALL NOT block the RT thread

  @AC-394.6
  Scenario: Module scan results are logged with count and any load errors
    Given the service has completed startup
    When module scan results are checked in the log
    Then the log SHALL contain the count of loaded modules and any load errors
