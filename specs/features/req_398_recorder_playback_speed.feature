@REQ-398 @product
Feature: Flight-Recorder Playback Speed Control — Replay at N× Speed

  @AC-398.1
  Scenario: Playback speed is configurable between 0.1× and 10×
    Given the flight recorder playback engine
    When playback speed is set to any value in [0.1, 10.0]
    Then playback SHALL proceed at the requested speed

  @AC-398.2
  Scenario: 1× playback matches original timing within 1 ms per second
    Given a recording being replayed at 1× speed
    When playback timing is measured over one second
    Then the deviation from real time SHALL be less than 1 ms per second

  @AC-398.3
  Scenario: 10× playback completes 10× faster than real time
    Given a recording being replayed at 10× speed
    When the total playback duration is measured
    Then it SHALL complete approximately 10× faster than the original recording duration

  @AC-398.4
  Scenario: Playback speed change takes effect within one playback tick
    Given an in-progress playback session
    When the playback speed is changed
    Then the new speed SHALL take effect within one playback tick

  @AC-398.5
  Scenario: Speed greater than 1× skips intermediate samples to maintain sync
    Given a recording being replayed at speed > 1×
    When sample delivery is observed
    Then intermediate samples SHALL be skipped to maintain temporal synchronisation

  @AC-398.6
  Scenario: Speed less than 1× inserts interpolated samples for smooth slow motion
    Given a recording being replayed at speed < 1×
    When sample delivery is observed
    Then interpolated samples SHALL be inserted between original samples for smooth output
