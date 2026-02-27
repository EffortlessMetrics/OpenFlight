@REQ-468 @product
Feature: P3D SimConnect Adapter — Prepar3D SimConnect Integration  @AC-468.1
  Scenario: Adapter connects to P3D SimConnect using the standard SimConnect API
    Given Prepar3D is running on the host
    When the P3D adapter is enabled in config
    Then the adapter SHALL connect using the SimConnect API compatible with both P3D and MSFS  @AC-468.2
  Scenario: Aircraft state variables are subscribed and published as BusSnapshot
    Given the P3D SimConnect adapter is connected
    When P3D publishes aircraft state changes
    Then the adapter SHALL convert the data and publish a BusSnapshot on the flight-bus  @AC-468.3
  Scenario: P3D add-ons directory is supported for panel configs
    Given a P3D installation with an add-ons directory containing panel definitions
    When the panel configuration loader runs
    Then it SHALL search the P3D add-ons directory for panel config files  @AC-468.4
  Scenario: Both P3D version 4.x and 5.x are supported
    Given a SimConnect DLL from either P3D version 4 or version 5
    When the adapter initialises
    Then it SHALL successfully connect and operate with both major versions
