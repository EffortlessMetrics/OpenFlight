@REQ-384 @product
Feature: Diagnostic Bundle Generation for Support  @AC-384.1
  Scenario: flightctl bundle produces a ZIP with logs, config, metrics, and device list
    Given the service is running with devices connected
    When the user runs flightctl bundle
    Then a ZIP file SHALL be produced containing logs, configuration, metrics, and device list  @AC-384.2
  Scenario: Bundle includes system info, connected devices, and active profile
    Given a diagnostic bundle has been generated
    When the bundle contents are inspected
    Then it SHALL include OS version, RAM, CPU info, connected devices, and the active profile  @AC-384.3
  Scenario: Bundle does not include private user data beyond flight configuration
    Given a diagnostic bundle has been generated
    When the bundle is scanned for sensitive content
    Then it SHALL NOT contain browser history, credentials, or unrelated personal files  @AC-384.4
  Scenario: Bundle generation completes within 5 seconds
    Given the service is running normally
    When flightctl bundle is executed
    Then the bundle ZIP file SHALL be fully written within 5 seconds  @AC-384.5
  Scenario: Bundle filename includes a timestamp
    Given a diagnostic bundle has been generated
    When the filename is inspected
    Then it SHALL match the pattern openflight-bundle-<timestamp>.zip  @AC-384.6
  Scenario: Bundle file is written to the user home directory by default
    Given flightctl bundle is run without specifying an output path
    When the bundle is created
    Then the ZIP file SHALL be written to the current user home directory
