Feature: Device Quirk Database
  As a flight simulation enthusiast
  I want device quirk database
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Maintain quirk database
    Given the system is configured for device quirk database
    When the feature is exercised
    Then service maintains a database of known device quirks

  Scenario: Query by vendor and product ID
    Given the system is configured for device quirk database
    When the feature is exercised
    Then quirk database is queryable by vendor and product id

  Scenario: Auto-apply quirks on device connect
    Given the system is configured for device quirk database
    When the feature is exercised
    Then quirks are applied automatically when a matching device is connected

  Scenario: Extensible via config files
    Given the system is configured for device quirk database
    When the feature is exercised
    Then quirk database is extensible via configuration files
