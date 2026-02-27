@REQ-268 @product
Feature: Service can record and replay axis input streams with microsecond-precision timestamps  @AC-268.1
  Scenario: Axis input stream recorded to binary file
    Given the flightd service is running and at least one axis is active
    When the user starts a recording session
    Then the service SHALL write axis samples to a binary recording file for the duration of the session  @AC-268.2
  Scenario: Recording file contains microsecond-precision timestamps
    Given a completed recording file
    When the file is inspected
    Then each sample record SHALL include a timestamp field with microsecond precision  @AC-268.3
  Scenario: Playback replays at original timing
    Given a recording file captured at known sample intervals
    When playback is started
    Then the service SHALL re-emit each sample at the same relative timing as recorded, within 1 ms tolerance  @AC-268.4
  Scenario: Playback is deterministic across runs
    Given the same recording file played back twice
    When both playback runs complete
    Then the sequence and values of emitted axis samples SHALL be identical for both runs  @AC-268.5
  Scenario: Recording started and stopped via CLI
    Given the flightd service is running
    When the user runs flightctl record start followed by flightctl record stop
    Then the service SHALL start recording on the first command and flush and close the file on the second  @AC-268.6
  Scenario: Recording file includes format version field
    Given a newly created recording file
    When the file header is parsed
    Then the header SHALL contain a version field that identifies the recording format revision
