@REQ-1019
Feature: WebSocket Events
  @AC-1019.1
  Scenario: Real-time events are available via WebSocket connection
    Given the system is configured for REQ-1019
    When the feature condition is met
    Then real-time events are available via websocket connection

  @AC-1019.2
  Scenario: Events include device state changes, axis updates, and profile switches
    Given the system is configured for REQ-1019
    When the feature condition is met
    Then events include device state changes, axis updates, and profile switches

  @AC-1019.3
  Scenario: WebSocket clients can subscribe to specific event categories
    Given the system is configured for REQ-1019
    When the feature condition is met
    Then websocket clients can subscribe to specific event categories

  @AC-1019.4
  Scenario: Connection handles reconnection with event replay for missed messages
    Given the system is configured for REQ-1019
    When the feature condition is met
    Then connection handles reconnection with event replay for missed messages
