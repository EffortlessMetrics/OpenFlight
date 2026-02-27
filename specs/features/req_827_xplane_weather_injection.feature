Feature: X-Plane Weather Injection
  As a flight simulation enthusiast
  I want x-plane weather injection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Inject weather via dataref writes
    Given the system is configured for x-plane weather injection
    When the feature is exercised
    Then x-Plane adapter supports injecting custom weather data via dataref writes

  Scenario: Include wind, speed, and turbulence
    Given the system is configured for x-plane weather injection
    When the feature is exercised
    Then injected weather includes wind direction, speed, and turbulence level

  Scenario: Enable/disable injection at runtime
    Given the system is configured for x-plane weather injection
    When the feature is exercised
    Then weather injection can be enabled or disabled at runtime

  Scenario: No interference when disabled
    Given the system is configured for x-plane weather injection
    When the feature is exercised
    Then injection does not interfere with X-Plane native weather when disabled
