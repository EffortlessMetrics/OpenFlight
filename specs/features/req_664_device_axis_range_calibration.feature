Feature: Device Axis Range Calibration
  As a flight simulation enthusiast
  I want the service to calibrate device axis physical range on demand
  So that each device axis is accurately mapped to its full movement range

  Background:
    Given the OpenFlight service is running with a supported device connected

  Scenario: Calibration captures min and max of each axis during movement
    When a calibration session is started and the user moves each axis through its full range
    Then the calibration records the minimum and maximum raw values observed

  Scenario: Calibration stores center position for axes that have one
    Given an axis supports a centre detent position
    When the user moves the axis to centre during calibration
    Then the calibration stores the observed centre raw value

  Scenario: Center is calibrated by pressing dedicated button or command
    When the user presses the designated centre calibration button or CLI command
    Then the current axis position is recorded as the centre calibration point

  Scenario: Calibration result replaces previous calibration for that device
    Given a device has an existing calibration record
    When a new calibration session is completed
    Then the new calibration result replaces the previous record for that device
