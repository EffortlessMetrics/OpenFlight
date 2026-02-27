@REQ-66
Feature: gRPC IPC layer

  @AC-66.1
  Scenario: IPC device message survives a protobuf round-trip
    Given a Device message with id, capabilities, health, and status set
    When the message is encoded with prost and decoded back
    Then all fields SHALL be preserved and the decoded message SHALL equal the original

  @AC-66.2
  Scenario: Protocol version strings are parsed and validated
    Given valid version strings like "1.2.3" and malformed strings like "1.2" or "a.b.c"
    When each string is parsed as a ProtocolVersion
    Then valid strings SHALL succeed and malformed strings SHALL return a parse error

  @AC-66.3
  Scenario: Removed RPCs and messages are detected as breaking changes
    Given a baseline schema and a new schema that removes an RPC and a message type
    When breaking-change detection runs
    Then each removal SHALL be reported as a breaking change
    And a schema with no removals SHALL report no breaking changes

  @AC-66.4
  Scenario: Feature negotiation selects the intersection of client and server features
    Given a client advertising features A and B and a server supporting features B and C
    When negotiate_features is called
    Then the negotiated set SHALL contain only feature B

  @AC-66.5
  Scenario: Fuzz inputs to IPC handlers do not cause panics
    Given arbitrary byte sequences as IPC message payloads
    When the payloads are fed to negotiate_features, apply_profile, and health_subscribe handlers
    Then no handler SHALL panic regardless of the input content
