@REQ-126 @infra
Feature: IPC protocol negotiation full pipeline

  @AC-126.1
  Scenario: Client with matching protocol version connects successfully
    Given an IPC service advertising protocol version "2.0.0"
    When a client with protocol version "2.0.0" attempts to connect
    Then negotiation SHALL succeed
    And the established connection SHALL use version "2.0.0"

  @AC-126.2
  Scenario: Client with higher minor version accepted (backward compatible)
    Given an IPC service advertising protocol version "2.0.0"
    When a client with protocol version "2.1.0" attempts to connect
    Then negotiation SHALL succeed
    And the established connection SHALL use the service's version "2.0.0"

  @AC-126.3
  Scenario: Client with incompatible major version is rejected
    Given an IPC service advertising protocol version "2.0.0"
    When a client with protocol version "1.0.0" attempts to connect
    Then the service SHALL reject the connection
    And the rejection SHALL include a version-mismatch error code

  @AC-126.4
  Scenario: NegotiateFeatures lists all available capabilities
    Given an IPC service with capabilities "axis_control", "ffb_control", and "profile_management"
    When a client sends a NegotiateFeatures request without specifying any filter
    Then the response SHALL list all three available capabilities

  @AC-126.5
  Scenario: Optional feature request: client gets only the subset it requested
    Given an IPC service with capabilities "axis_control", "ffb_control", and "profile_management"
    When a client sends a NegotiateFeatures request for only "axis_control" and "ffb_control"
    Then the response SHALL list exactly "axis_control" and "ffb_control"
    And "profile_management" SHALL NOT appear in the response

  @AC-126.6
  Scenario: Connection state transitions from new through negotiating to established
    Given a new IPC connection in the "new" state
    When the version negotiation handshake is initiated
    Then the connection state SHALL transition to "negotiating"
    When the handshake completes successfully
    Then the connection state SHALL transition to "established"
    And no backwards state transitions SHALL occur
