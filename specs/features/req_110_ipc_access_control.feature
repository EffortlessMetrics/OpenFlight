@REQ-110
Feature: IPC access control

  @AC-110.1
  Scenario: IPC connection from a permitted user is accepted
    Given an IPC ACL configuration that lists the connecting user as permitted
    When the connection is validated against the ACL
    Then the ACL check SHALL accept the connection

  @AC-110.2
  Scenario: IPC connection from an unpermitted user is rejected
    Given an IPC ACL configuration that does not list the connecting user
    When the connection is validated against the ACL
    Then the ACL check SHALL reject the connection

  @AC-110.3
  Scenario: Security status is queryable via GetSecurityStatus RPC
    Given a GetSecurityStatus RPC response with security fields populated
    When the response is encoded with prost and decoded back
    Then all security status fields SHALL be preserved and equal to the originals
