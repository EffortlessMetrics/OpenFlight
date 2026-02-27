@REQ-533 @product
Feature: Device Axis Calibration Persistence — Calibration Survives Firmware Updates  @AC-533.1
  Scenario: Calibration is keyed by VID, PID, and serial number
    Given a calibration entry for VID=0x044F PID=0xB10A serial=SN001
    When the same device reconnects
    Then the stored calibration SHALL be retrieved using the VID/PID/serial key  @AC-533.2
  Scenario: Calibration survives firmware version changes
    Given a device with stored calibration updates its firmware
    When the device reconnects with a new firmware version but the same VID/PID/serial
    Then the existing calibration SHALL remain valid and be applied  @AC-533.3
  Scenario: Calibration is invalidated when axis count changes
    Given a stored calibration for a device with 6 axes
    When the device reconnects and now reports 8 axes
    Then the stored calibration SHALL be invalidated and a recalibration flag set  @AC-533.4
  Scenario: Stale calibration triggers a recalibration prompt
    Given a device that has had its calibration invalidated
    When the user runs `flightctl status`
    Then the output SHALL warn that recalibration is required for the device
