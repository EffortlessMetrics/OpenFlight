Feature: CLI Remote Service
  As a flight simulation enthusiast
  I want the CLI to support connecting to remote service instances
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Connect to remote service
    Given a remote service instance is running
    When I run flightctl with --host remote:5000 status
    Then the CLI connects to the remote service

  Scenario: TLS encryption used
    Given a remote connection is established
    When data is exchanged
    Then the connection uses gRPC with TLS encryption

  Scenario: Host configurable via flag or env
    Given the remote host is configured
    When the CLI starts
    Then it uses the host from the flag or OPENFLIGHT_HOST environment variable

  Scenario: Configurable connection timeout
    Given a remote connection is attempted
    When the connection does not respond
    Then it times out after the configured duration
