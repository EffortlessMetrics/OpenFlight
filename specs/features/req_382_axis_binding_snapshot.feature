@REQ-382 @product
Feature: Axis Binding Snapshot Export  @AC-382.1
  Scenario: flightctl axis snapshot writes a file with current axis states
    Given the service is running with active axes
    When the user runs flightctl axis snapshot
    Then a YAML or JSON file SHALL be written containing current axis states  @AC-382.2
  Scenario: Snapshot includes all required axis fields
    Given axes with calibration and curve configuration active
    When a snapshot is taken
    Then each entry SHALL include axis ID, raw value, processed value, deadzone, and curve config  @AC-382.3
  Scenario: Snapshot file is human-readable and loadable for debugging
    Given a snapshot file produced by flightctl axis snapshot
    When the file is opened in a text editor
    Then the content SHALL be readable and parseable without special tools  @AC-382.4
  Scenario: Snapshot command exits within 100 ms
    Given the service is running normally
    When flightctl axis snapshot is executed
    Then the command SHALL complete and exit within 100 ms  @AC-382.5
  Scenario: Snapshot handles axes with no active input
    Given axes that have not received input since startup
    When a snapshot is taken
    Then those axes SHALL appear in the snapshot file with zero values  @AC-382.6
  Scenario: Snapshot filename includes a timestamp
    Given the user runs flightctl axis snapshot
    When the output file is created
    Then the filename SHALL include a timestamp in ISO 8601 format
