Feature: Device Enumeration Cache
  As a flight simulation enthusiast
  I want the service to cache device enumeration results
  So that repeated enumeration is fast and hotplug events are handled efficiently

  Background:
    Given the OpenFlight service is running

  Scenario: Device enumeration runs at startup and on hotplug
    When the service starts
    Then a full device enumeration is performed and cached in memory
    When a new HID device is connected
    Then a device enumeration is triggered and the cache is updated

  Scenario: Cache is stored in memory with configurable TTL
    Given the device cache TTL is configured to 60 seconds
    When 60 seconds pass without a hotplug event
    Then the cache is considered stale and a re-enumeration is triggered

  Scenario: Forced re-enumeration is available via CLI
    When the command "flightctl devices refresh" is run
    Then a forced device re-enumeration is performed and the cache is updated

  Scenario: Cache state is visible in diagnostics
    When the command "flightctl diagnostics devices" is run
    Then the output includes the cache age and number of cached devices
