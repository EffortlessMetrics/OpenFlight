@REQ-1014
Feature: Replay Sharing
  @AC-1014.1
  Scenario: Flight input recordings can be exported as shareable replay files
    Given the system is configured for REQ-1014
    When the feature condition is met
    Then flight input recordings can be exported as shareable replay files

  @AC-1014.2
  Scenario: Replay files include all axis and button data with timestamps
    Given the system is configured for REQ-1014
    When the feature condition is met
    Then replay files include all axis and button data with timestamps

  @AC-1014.3
  Scenario: Shared replays can be loaded for playback on any compatible installation
    Given the system is configured for REQ-1014
    When the feature condition is met
    Then shared replays can be loaded for playback on any compatible installation

  @AC-1014.4
  Scenario: Replay file format is versioned for forward compatibility
    Given the system is configured for REQ-1014
    When the feature condition is met
    Then replay file format is versioned for forward compatibility
