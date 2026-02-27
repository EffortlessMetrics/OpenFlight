@REQ-445 @product
Feature: DCS Export Script Integration — Manage Export.lua Script Deployment

  @AC-445.1
  Scenario: Service can deploy Export.lua to DCS user scripts directory
    Given DCS World is installed and the user scripts directory is known
    When the deploy command is issued
    Then the bundled Export.lua SHALL be copied to the DCS user scripts directory

  @AC-445.2
  Scenario: Service verifies Export.lua hash matches expected version
    Given Export.lua exists in the DCS scripts directory
    When the service starts or a verify command is issued
    Then it SHALL compute the file hash and compare it against the expected hash for the bundled version

  @AC-445.3
  Scenario: Service detects conflicting Export.lua from other applications
    Given an Export.lua from a third-party application is already present
    When the service checks the scripts directory
    Then it SHALL detect the conflict and report which application owns the existing script

  @AC-445.4
  Scenario: Deployment can be triggered via flightctl dcs install-export-script
    Given the flightctl CLI is installed
    When the user runs flightctl dcs install-export-script
    Then the Export.lua SHALL be deployed and a success message SHALL be displayed
