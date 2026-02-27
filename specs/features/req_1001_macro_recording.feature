@REQ-1001
Feature: Macro Recording
  @AC-1001.1
  Scenario: WHEN user starts macro recording THEN all input events SHALL be captured with timestamps
    Given the system is configured for REQ-1001
    When the feature condition is met
    Then when user starts macro recording then all input events shall be captured with timestamps

  @AC-1001.2
  Scenario: Recorded macro can be saved with a user-defined name
    Given the system is configured for REQ-1001
    When the feature condition is met
    Then recorded macro can be saved with a user-defined name

  @AC-1001.3
  Scenario: Macro playback reproduces input sequence with original timing
    Given the system is configured for REQ-1001
    When the feature condition is met
    Then macro playback reproduces input sequence with original timing

  @AC-1001.4
  Scenario: Macro recording can be stopped and discarded without side effects
    Given the system is configured for REQ-1001
    When the feature condition is met
    Then macro recording can be stopped and discarded without side effects
