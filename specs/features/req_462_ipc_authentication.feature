@REQ-462 @product
Feature: IPC Authentication — Optional Auth Token for IPC Connections

  @AC-462.1
  Scenario: IPC server enforces auth token when configured
    Given a service configured with auth_token = "secret123"
    When a client connects and sends a request without providing the token
    Then the server SHALL reject the request with UNAUTHENTICATED gRPC status

  @AC-462.2
  Scenario: Auth token is configurable in service config
    Given a service config file with the ipc.auth_token field set
    When the service starts
    Then the IPC server SHALL require that token for all incoming connections

  @AC-462.3
  Scenario: Authenticated clients with valid token can make requests
    Given a service configured with auth_token = "secret123"
    When a client connects and provides the correct token in request metadata
    Then the request SHALL be processed normally and a valid response returned

  @AC-462.4
  Scenario: Auth requirement is disabled by default for local connections
    Given a service started with no auth_token configured
    When a local client connects without providing any token
    Then the connection SHALL succeed and requests SHALL be processed without authentication
