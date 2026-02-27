@REQ-520 @product
Feature: Simulator Version Compatibility Check

  @AC-520.1 @AC-520.2
  Scenario: Service verifies simulator version against compat manifest on connect
    Given the compat manifest specifies supported MSFS versions 1.30.0 through 1.35.0
    When the service connects to MSFS version 1.32.0
    Then the version check SHALL pass with no warning logged

  @AC-520.3
  Scenario: Unsupported simulator version logs a warning but does not block
    Given the compat manifest does not include the connected simulator version
    When the service attempts to connect to the simulator
    Then a warning SHALL be logged but the connection SHALL proceed

  @AC-520.4
  Scenario: Version check result is reported in flightctl sim status
    Given the service is connected to a simulator
    When the user runs flightctl sim status
    Then the output SHALL include the simulator version and its compat check result
