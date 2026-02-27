@REQ-310 @product
Feature: MSFS 2024 Compatibility

  @AC-310.1
  Scenario: Service detects MSFS 2024 via SimConnect client version
    Given MSFS 2024 is running
    When the service connects via SimConnect
    Then the service SHALL detect the MSFS 2024 version by inspecting the SimConnect client version information

  @AC-310.2
  Scenario: MSFS 2024 SimConnect variables are mapped correctly
    Given the service has detected MSFS 2024
    When the service subscribes to SimConnect simulation variables
    Then the service SHALL use the correct MSFS 2024 variable mappings for all subscribed data definitions

  @AC-310.3
  Scenario: FFB effects work with MSFS 2024 using the same SimConnect FFB API
    Given the service is connected to MSFS 2024
    When force feedback effects are triggered
    Then the service SHALL deliver FFB effects through the SimConnect FFB API which is unchanged between MSFS 2020 and 2024

  @AC-310.4
  Scenario: No breaking changes for MSFS 2020 users
    Given the service is configured for an MSFS 2020 session
    When the service connects to MSFS 2020 via SimConnect
    Then all existing MSFS 2020 features SHALL continue to work without modification or reconfiguration

  @AC-310.5
  Scenario: MSFS 2024-specific features are detected via capability check
    Given the service has connected to a Microsoft Flight Simulator instance
    When the service performs a capability check
    Then the service SHALL detect and enable MSFS 2024-specific features only when those capabilities are reported as available

  @AC-310.6
  Scenario: CI integration test validates MSFS 2024 SimConnect schema
    Given the MSFS 2024 SimConnect schema definition is available in CI
    When the CI integration test runs the schema validation suite
    Then all variable definitions used by the service SHALL conform to the MSFS 2024 SimConnect schema
