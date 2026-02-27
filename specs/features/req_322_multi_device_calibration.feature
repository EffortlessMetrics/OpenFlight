@REQ-322 @product
Feature: Multi-Device Calibration  @AC-322.1
  Scenario: Calibration can be run for all connected axes simultaneously
    Given multiple HID devices with axes are connected
    When the user runs flightctl calibrate without specifying a device
    Then the service SHALL begin calibration for all detected axes across all connected devices  @AC-322.2
  Scenario: Calibration procedure guides user through min/max/center positions
    Given a calibration session is active for an axis
    When the calibration procedure runs
    Then the CLI SHALL prompt the user to move the axis to its minimum, maximum, and center positions in sequence  @AC-322.3
  Scenario: Calibration results are stored per device using VID and PID
    Given calibration has been completed for a device with VID 0x1234 and PID 0x5678
    When the results are persisted
    Then the calibration store SHALL record the results keyed by VID/PID  @AC-322.4
  Scenario: Stored calibration is applied automatically on device connect
    Given calibration data exists in the store for a device identified by VID/PID
    When that device is connected to the system
    Then the service SHALL automatically apply the stored calibration to all axes of that device  @AC-322.5
  Scenario: CLI calibrate command accepts an optional device argument
    Given the service is running
    When the user runs flightctl calibrate <device>
    Then the calibration procedure SHALL be scoped to the specified device only  @AC-322.6
  Scenario: Calibration times out after 30 seconds per axis with progress display
    Given a calibration session is active
    When the user does not move an axis within 30 seconds
    Then the CLI SHALL display a progress indicator and abort calibration for that axis with a timeout error
