Feature: CLI Diagnostic Bundle
  As a flight simulation enthusiast
  I want the CLI to generate a complete diagnostic bundle
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Bundle includes logs and config
    Given the service is running
    When I run the diagnostic bundle command
    Then a ZIP file is generated containing logs, config, and state

  Scenario: Bundle includes device info
    Given devices are connected
    When the diagnostic bundle is generated
    Then it includes device enumeration and connection status

  Scenario: Sensitive data is redacted
    Given the service has sensitive configuration
    When the diagnostic bundle is generated
    Then sensitive data such as keys and tokens is redacted

  Scenario: Generation completes within 10s
    Given the service is running
    When the diagnostic bundle is generated
    Then generation completes within 10 seconds
