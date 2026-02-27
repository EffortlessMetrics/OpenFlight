@REQ-438 @product
Feature: IPC Protocol Version Negotiation — Negotiate Protocol Version on Connection

  @AC-438.1
  Scenario: Server announces supported IPC protocol versions on connect
    Given a client establishes a new IPC connection
    When the handshake message is received
    Then it SHALL contain the list of IPC protocol versions supported by the server

  @AC-438.2
  Scenario: Client selects highest mutually supported version
    Given the server supports versions [1, 2, 3] and the client supports [2, 3, 4]
    When version negotiation completes
    Then the negotiated version SHALL be 3

  @AC-438.3
  Scenario: Connection is rejected with clear error if no compatible version exists
    Given the server supports versions [1, 2] and the client supports only [3]
    When version negotiation is attempted
    Then the connection SHALL be rejected with a version_mismatch error message

  @AC-438.4
  Scenario: Version negotiation completes within 100ms or connection times out
    Given a client initiates a connection
    When the negotiation handshake does not complete within 100ms
    Then the connection SHALL be closed with a negotiation_timeout error
