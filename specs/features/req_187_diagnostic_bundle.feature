@REQ-187 @infra
Feature: OpenFlight generates comprehensive diagnostic bundles on request  @AC-187.1
  Scenario: flightctl diag bundle creates a zip artifact
    Given the OpenFlight service is running
    When the user runs the command flightctl diag bundle
    Then a zip artifact SHALL be created at the path reported by the CLI  @AC-187.2
  Scenario: Bundle includes profile, device list, and axis counters
    Given a diagnostic bundle has been generated
    When the bundle contents are inspected
    Then the bundle SHALL include the current profile, connected device list, and axis counters  @AC-187.3
  Scenario: Bundle includes last 1000 lines of structured log
    Given a diagnostic bundle has been generated
    When the bundle log file is inspected
    Then the bundle SHALL contain at least the last 1000 lines of the structured log  @AC-187.4
  Scenario: Bundle includes system information
    Given a diagnostic bundle has been generated
    When the bundle system info section is inspected
    Then the bundle SHALL include OS version, driver versions, and CPU affinity configuration  @AC-187.5
  Scenario: Sensitive data redacted from bundle
    Given a diagnostic bundle has been generated from a system with credentials or serial numbers present
    When the bundle is scanned for sensitive data
    Then credentials and hardware serial numbers SHALL be redacted from all bundle contents  @AC-187.6
  Scenario: Bundle generation completes within time limit
    Given the OpenFlight service is running on normal hardware
    When the user runs flightctl diag bundle
    Then the bundle SHALL be fully generated and written within 5 seconds
