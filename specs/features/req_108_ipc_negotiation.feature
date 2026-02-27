@REQ-108
Feature: IPC protocol negotiation

  @AC-108.1
  Scenario: Client and service negotiate to a mutually compatible version
    Given a client advertising protocol version "1.2.0" and a service supporting "1.3.0"
    When version compatibility is checked
    Then negotiation SHALL succeed and the highest mutually compatible version SHALL be selected

  @AC-108.2
  Scenario: Incompatible client version is rejected with a version-mismatch error
    Given a client whose protocol version is incompatible with the service
    When a connection attempt is made
    Then the service SHALL reject the connection with a version-mismatch IPC error

  @AC-108.3
  Scenario: NegotiateFeatures message survives a protobuf round-trip
    Given a NegotiateFeatures request with a set of client feature flags
    When the message is encoded with prost and decoded back
    Then all feature flags SHALL be preserved and the decoded message SHALL equal the original
