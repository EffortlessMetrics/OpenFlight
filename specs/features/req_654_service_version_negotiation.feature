Feature: Service Version Negotiation
  As a flight simulation enthusiast
  I want the CLI and service to negotiate protocol versions
  So that version mismatches are detected early with a helpful error

  Background:
    Given the OpenFlight service is running

  Scenario: Service reports its protocol version in handshake
    When the CLI connects to the service
    Then the service includes its protocol version in the initial handshake response

  Scenario: CLI rejects connection to incompatible service version
    Given the CLI requires protocol version 2
    And the service reports protocol version 1
    When the CLI attempts to connect
    Then the connection is rejected with a version incompatibility error

  Scenario: Version negotiation error has a helpful message
    Given the CLI and service have incompatible protocol versions
    When the CLI attempts to connect
    Then the error message states the CLI version, service version, and how to resolve the mismatch

  Scenario: Protocol versions are documented in IPC guide
    When the IPC documentation is reviewed
    Then the protocol version history and compatibility matrix are present in the IPC guide
