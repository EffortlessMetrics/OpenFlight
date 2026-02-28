Feature: Dependency Auditing
  As a flight simulation enthusiast
  I want dependency auditing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Automated vulnerability scanning checks all dependencies on every build
    Given the system is configured for dependency auditing
    When the feature is exercised
    Then automated vulnerability scanning checks all dependencies on every build

  Scenario: Known vulnerabilities are flagged with severity level and remediation guidance
    Given the system is configured for dependency auditing
    When the feature is exercised
    Then known vulnerabilities are flagged with severity level and remediation guidance

  Scenario: Audit results block release when critical or high severity issues are found
    Given the system is configured for dependency auditing
    When the feature is exercised
    Then audit results block release when critical or high severity issues are found

  Scenario: Dependency audit history is retained for compliance reporting
    Given the system is configured for dependency auditing
    When the feature is exercised
    Then dependency audit history is retained for compliance reporting