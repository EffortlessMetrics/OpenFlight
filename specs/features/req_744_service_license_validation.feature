Feature: Service License Validation
  As a flight simulation enthusiast
  I want the service to validate license key on startup
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: License validated on startup
    Given a license key is configured
    When the service starts
    Then the license key is validated

  Scenario: Invalid license limits to free tier
    Given the license key is invalid
    When validation fails
    Then the service operates with free-tier features only

  Scenario: Expiry warned 30 days ahead
    Given the license expires within 30 days
    When the service starts
    Then a warning about upcoming expiry is shown

  Scenario: Status queryable via CLI and IPC
    Given the service is running
    When license status is queried
    Then the current license status is returned
