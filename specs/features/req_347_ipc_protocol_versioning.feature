@REQ-347 @product
Feature: IPC Protocol Versioning  @AC-347.1
  Scenario: gRPC service version is included in every response
    Given the service is running
    When any gRPC response is received by a client
    Then the response metadata SHALL include the server protocol version  @AC-347.2
  Scenario: Client must send its version in request metadata
    Given a gRPC client connects to the service
    When the client sends a request without a version in the metadata
    Then the service SHALL reject the request with a clear version-missing error  @AC-347.3
  Scenario: Version mismatch produces a clear error
    Given a client sends a request with an incompatible protocol version
    When the service processes the request
    Then the service SHALL return a descriptive version mismatch error rather than a silent failure  @AC-347.4
  Scenario: Major version bump requires explicit client upgrade
    Given the server is running protocol major version N+1
    When a client on major version N connects
    Then the service SHALL reject the client with an explicit upgrade-required message  @AC-347.5
  Scenario: Minor version differences are backward compatible
    Given the server is running protocol version 2.3 and a client sends version 2.1
    When the client sends a request
    Then the service SHALL process the request successfully  @AC-347.6
  Scenario: Version info is visible via flightctl version
    Given the service is running
    When the user runs "flightctl version"
    Then the command output SHALL include the IPC protocol version in use
