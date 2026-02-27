@REQ-480 @product
Feature: Service IPC Version Reporting — Handshake Version Exchange  @AC-480.1
  Scenario: Connect response includes service version string
    Given the service is running
    When a client sends a Connect request via gRPC
    Then the ConnectResponse SHALL include the service semantic version string  @AC-480.2
  Scenario: Connect response includes supported API capabilities
    Given the service is running
    When a client sends a Connect request via gRPC
    Then the ConnectResponse SHALL include a list of supported API capability identifiers  @AC-480.3
  Scenario: Client can detect version mismatch and warn user
    Given a client whose minimum required service version is higher than the running service version
    When the client receives the ConnectResponse
    Then the client SHALL log a warning indicating the version mismatch before proceeding  @AC-480.4
  Scenario: Version is accessible via flightctl version command
    Given the service is running
    When `flightctl version` is executed
    Then the command SHALL display both the CLI version and the connected service version
