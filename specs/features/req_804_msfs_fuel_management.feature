Feature: MSFS Fuel Management
  As a flight simulation enthusiast
  I want msfs fuel management
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Read fuel tank quantities for all tanks
    Given the system is configured for msfs fuel management
    When the feature is exercised
    Then simConnect adapter reads fuel tank quantities for all aircraft tanks

  Scenario: Expose fuel flow rate per engine
    Given the system is configured for msfs fuel management
    When the feature is exercised
    Then fuel flow rate per engine is exposed as a real-time variable

  Scenario: Publish fuel state changes to event bus
    Given the system is configured for msfs fuel management
    When the feature is exercised
    Then fuel system state changes are published to the event bus

  Scenario: Fuel refresh rate matches polling interval
    Given the system is configured for msfs fuel management
    When the feature is exercised
    Then fuel data refresh rate matches the configured SimConnect polling interval
