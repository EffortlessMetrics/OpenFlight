@REQ-1035
Feature: File Association
  @AC-1035.1
  Scenario: OpenFlight profile files (.ofp) are associated with the application
    Given the system is configured for REQ-1035
    When the feature condition is met
    Then openflight profile files (.ofp) are associated with the application

  @AC-1035.2
  Scenario: Double-clicking a .ofp file opens it in the profile editor
    Given the system is configured for REQ-1035
    When the feature condition is met
    Then double-clicking a .ofp file opens it in the profile editor

  @AC-1035.3
  Scenario: File association is registered during installation
    Given the system is configured for REQ-1035
    When the feature condition is met
    Then file association is registered during installation

  @AC-1035.4
  Scenario: File association can be repaired via CLI command
    Given the system is configured for REQ-1035
    When the feature condition is met
    Then file association can be repaired via cli command
