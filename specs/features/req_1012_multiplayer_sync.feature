@REQ-1012
Feature: Multiplayer Sync
  @AC-1012.1
  Scenario: Control settings can be synchronized across crew station instances
    Given the system is configured for REQ-1012
    When the feature condition is met
    Then control settings can be synchronized across crew station instances

  @AC-1012.2
  Scenario: Sync protocol handles network latency with interpolation
    Given the system is configured for REQ-1012
    When the feature condition is met
    Then sync protocol handles network latency with interpolation

  @AC-1012.3
  Scenario: Each crew station maintains independent safety interlocks
    Given the system is configured for REQ-1012
    When the feature condition is met
    Then each crew station maintains independent safety interlocks

  @AC-1012.4
  Scenario: Sync conflicts are resolved by station priority assignment
    Given the system is configured for REQ-1012
    When the feature condition is met
    Then sync conflicts are resolved by station priority assignment
