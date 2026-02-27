Feature: Service Telemetry Sampling
  As a flight simulation enthusiast
  I want service telemetry sampling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable sampling per metric
    Given the system is configured for service telemetry sampling
    When the feature is exercised
    Then service supports configurable telemetry sampling rates per metric

  Scenario: Runtime rate changes
    Given the system is configured for service telemetry sampling
    When the feature is exercised
    Then sampling rate can be changed at runtime without restart

  Scenario: Default 1-in-N for high frequency
    Given the system is configured for service telemetry sampling
    When the feature is exercised
    Then high-frequency metrics default to 1-in-n sampling

  Scenario: Persist sampling config
    Given the system is configured for service telemetry sampling
    When the feature is exercised
    Then sampling configuration is persisted across restarts
