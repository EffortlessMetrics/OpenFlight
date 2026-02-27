Feature: MSFS SimBridge Integration
  As a flight simulation enthusiast
  I want the SimConnect adapter to support MSFS SimBridge
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Connect via SimBridge
    Given MSFS SimBridge is running
    When the SimConnect adapter starts
    Then it connects to MSFS via SimBridge

  Scenario: SimBridge auto-detected
    Given SimBridge is available on the network
    When the adapter scans for connections
    Then SimBridge is auto-detected

  Scenario: Fallback to direct SimConnect
    Given SimBridge is not available
    When the adapter fails to connect via SimBridge
    Then it falls back to direct SimConnect

  Scenario: Connection status exposed
    Given the adapter is connected via SimBridge
    When a client queries connection status via IPC
    Then the SimBridge connection status is reported
