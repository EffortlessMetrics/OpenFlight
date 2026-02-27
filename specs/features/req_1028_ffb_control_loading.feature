@REQ-1028
Feature: FFB Control Loading
  @AC-1028.1
  Scenario: Realistic control force simulation based on flight conditions
    Given the system is configured for REQ-1028
    When the feature condition is met
    Then realistic control force simulation based on flight conditions

  @AC-1028.2
  Scenario: Control loading force increases with airspeed per aircraft model
    Given the system is configured for REQ-1028
    When the feature condition is met
    Then control loading force increases with airspeed per aircraft model

  @AC-1028.3
  Scenario: Trim position affects the neutral force balance point
    Given the system is configured for REQ-1028
    When the feature condition is met
    Then trim position affects the neutral force balance point

  @AC-1028.4
  Scenario: Control loading respects maximum force limits from safety envelope
    Given the system is configured for REQ-1028
    When the feature condition is met
    Then control loading respects maximum force limits from safety envelope
