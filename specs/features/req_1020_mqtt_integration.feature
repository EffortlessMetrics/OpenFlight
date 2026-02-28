@REQ-1020
Feature: MQTT Integration
  @AC-1020.1
  Scenario: Device and axis events can be published to MQTT broker
    Given the system is configured for REQ-1020
    When the feature condition is met
    Then device and axis events can be published to mqtt broker

  @AC-1020.2
  Scenario: MQTT topic structure follows a documented hierarchy
    Given the system is configured for REQ-1020
    When the feature condition is met
    Then mqtt topic structure follows a documented hierarchy

  @AC-1020.3
  Scenario: MQTT commands can trigger profile switches and device actions
    Given the system is configured for REQ-1020
    When the feature condition is met
    Then mqtt commands can trigger profile switches and device actions

  @AC-1020.4
  Scenario: Broker connection supports TLS and credential-based authentication
    Given the system is configured for REQ-1020
    When the feature condition is met
    Then broker connection supports tls and credential-based authentication
