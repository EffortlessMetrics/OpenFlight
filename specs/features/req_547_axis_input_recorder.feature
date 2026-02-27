@REQ-547 @product
Feature: Axis Input Recorder — Service should record axis inputs for replay and debugging

  @AC-547.1
  Scenario: Axis recorder captures timestamped axis values to a binary file
    Given the axis input recorder is enabled
    When axis values are received at the RT spine
    Then each value SHALL be written to a binary file with a monotonic timestamp

  @AC-547.2
  Scenario: Recording is started and stopped via CLI command
    Given the service is running
    When the operator runs the start-recording CLI command
    Then the recorder SHALL begin capturing axis data
    And when the stop-recording command is issued the recorder SHALL flush and close the file

  @AC-547.3
  Scenario: Recorded file has configurable duration limit
    Given a recording duration limit of 60 seconds is configured
    When recording has been active for 60 seconds
    Then the recorder SHALL automatically stop and close the file

  @AC-547.4
  Scenario: Recording can be replayed as if from a live device
    Given a previously recorded binary axis file
    When the replay command is issued
    Then the service SHALL feed the recorded axis values into the axis pipeline as if from a live device
