@REQ-429 @product
Feature: Diagnostic Bundle Generation — Generate ZIP Bundle for Support

  @AC-429.1
  Scenario: Diagnostic bundle includes logs, config, and metrics snapshot
    Given the service is running
    When a diagnostic bundle is generated
    Then the ZIP archive SHALL contain current logs, active config, and a metrics snapshot

  @AC-429.2
  Scenario: Bundle is written to user-specified or default output path
    Given a user invokes the diagnostic command with an explicit output path
    When the bundle is generated
    Then the ZIP file SHALL be written to the specified path
    And if no path is specified it SHALL be written to the default output location

  @AC-429.3
  Scenario: Bundle includes system info including OS, version, and connected devices
    Given a generated diagnostic bundle
    When its contents are inspected
    Then a system_info entry SHALL include OS name, OpenFlight version, and connected device list

  @AC-429.4
  Scenario: Bundle generation is triggered via CLI diagnostic command
    Given the service is running
    When `flightctl diagnostic bundle` is executed
    Then a diagnostic bundle SHALL be generated and its path printed to stdout
