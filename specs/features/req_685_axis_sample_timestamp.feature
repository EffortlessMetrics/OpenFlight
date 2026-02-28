@REQ-685
Feature: Axis Sample Timestamp
  @AC-685.1
  Scenario: HID sample timestamps are propagated through pipeline
    Given the system is configured for REQ-685
    When the feature condition is met
    Then hid sample timestamps are propagated through pipeline

  @AC-685.2
  Scenario: Timestamp resolution is at minimum 1ms
    Given the system is configured for REQ-685
    When the feature condition is met
    Then timestamp resolution is at minimum 1ms

  @AC-685.3
  Scenario: Timestamps are included in bus snapshot
    Given the system is configured for REQ-685
    When the feature condition is met
    Then timestamps are included in bus snapshot

  @AC-685.4
  Scenario: Timestamp monotonicity is validated; non-monotonic timestamps are flagged
    Given the system is configured for REQ-685
    When the feature condition is met
    Then timestamp monotonicity is validated; non-monotonic timestamps are flagged
