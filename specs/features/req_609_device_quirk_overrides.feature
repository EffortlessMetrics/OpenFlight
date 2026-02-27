Feature: Device Quirk Override System
  As a flight simulation enthusiast
  I want to apply device quirk overrides at runtime
  So that I can fix device behaviour without modifying manifest files

  Background:
    Given the OpenFlight service is running

  Scenario: Quirk overrides can be applied without changing manifest files
    When the user applies a quirk override for a device
    Then the override takes effect without modifying any manifest file

  Scenario: Override is applied per VID/PID and persisted in user config
    When the user sets a quirk override for VID 0x044F PID 0xB10A
    Then the override is persisted in the user config file
    And the override is applied on the next service start

  Scenario: Override source is logged for diagnostics
    Given a quirk override is active for a device
    When the service starts
    Then the log contains an entry indicating the override source

  Scenario: flightctl devices quirks shows active overrides
    Given one or more quirk overrides are active
    When the user runs "flightctl devices quirks"
    Then the output lists all active quirk overrides with their VID/PID
