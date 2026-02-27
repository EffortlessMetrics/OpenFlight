@REQ-714
Feature: Axis Priority Arbitration
  @AC-714.1
  Scenario: When multiple inputs target the same axis priority determines winner
    Given the system is configured for REQ-714
    When the feature condition is met
    Then when multiple inputs target the same axis priority determines winner

  @AC-714.2
  Scenario: Priority is assigned per device in the profile
    Given the system is configured for REQ-714
    When the feature condition is met
    Then priority is assigned per device in the profile

  @AC-714.3
  Scenario: Higher priority input overrides lower priority completely
    Given the system is configured for REQ-714
    When the feature condition is met
    Then higher priority input overrides lower priority completely

  @AC-714.4
  Scenario: Priority ties are resolved by most-recent-input policy
    Given the system is configured for REQ-714
    When the feature condition is met
    Then priority ties are resolved by most-recent-input policy
