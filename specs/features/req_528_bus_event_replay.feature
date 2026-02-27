@REQ-528 @product
Feature: Bus Event Replay for Testing — Deterministic Playback of flight-bus Events  @AC-528.1
  Scenario: Bus events can be serialized to a replay file
    Given the bus recorder is active during a flight session
    When events are published to flight-bus
    Then each event SHALL be written to a structured replay file with a monotonic timestamp  @AC-528.2
  Scenario: Replay file can be played back as bus events in test mode
    Given a replay file captured from a previous session
    When the bus is started in replay mode with that file
    Then all recorded events SHALL be re-published to subscribers in the original order  @AC-528.3
  Scenario: Replay preserves event timing with configurable speed
    Given a replay file and a playback speed of 2.0x
    When replay begins
    Then inter-event delays SHALL be halved so the replay completes in half the original duration  @AC-528.4
  Scenario: Replay can be triggered via CLI command
    Given a replay file at a known path
    When `flightctl replay --file <path>` is executed
    Then the service SHALL enter replay mode and begin emitting events from the file
