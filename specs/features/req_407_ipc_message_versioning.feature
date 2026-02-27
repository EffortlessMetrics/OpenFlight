@REQ-407 @product
Feature: IPC Message Versioning — Handle Protocol Version Mismatches

  @AC-407.1
  Scenario: Every IPC message includes a protocol version field
    Given any IPC message sent between client and server
    When the message is inspected
    Then it SHALL contain a protocol version field

  @AC-407.2
  Scenario: Version mismatch returns VersionMismatch error
    Given a client with a protocol version incompatible with the server
    When the client sends a message
    Then the server SHALL return a VersionMismatch error

  @AC-407.3
  Scenario: Client includes its supported version range in connection
    Given a client initiating a connection
    When the connection handshake is observed
    Then the client message SHALL include its supported minimum and maximum protocol versions

  @AC-407.4
  Scenario: Server responds with its current version if within client's range
    Given a client whose supported version range includes the server's version
    When the version negotiation completes
    Then the server SHALL respond with its current version

  @AC-407.5
  Scenario: Version negotiation happens on first connection
    Given a new IPC connection being established
    When the first message is exchanged
    Then version negotiation SHALL occur before any other communication

  @AC-407.6
  Scenario: Integration test verifies behavior when client is one version ahead of server
    Given a client at protocol version N+1 and a server at version N
    When the client connects to the server
    Then the behavior SHALL match the defined version negotiation policy
