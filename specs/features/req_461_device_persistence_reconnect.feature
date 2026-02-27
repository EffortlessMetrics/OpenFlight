@REQ-461 @product
Feature: Device Persistence Across Reconnect — Restore Device Config on Reconnection

  @AC-461.1
  Scenario: Device is identified by stable ID from VID/PID and serial number
    Given a device with VID 0x044F, PID 0xB10A, and serial "SN-00123"
    When the device is enumerated
    Then its stable ID SHALL be derived from the VID, PID, and serial number combination

  @AC-461.2
  Scenario: Calibration and axis config are restored on reconnect
    Given a device previously connected with saved calibration and axis configuration
    When the device is disconnected and reconnected
    Then the calibration and axis configuration SHALL be automatically restored from the persistence store

  @AC-461.3
  Scenario: Reconnection is detected within 500ms via hotplug events
    Given a device that has just been physically reconnected
    When the OS fires a hotplug event
    Then the service SHALL detect and process the reconnection within 500 milliseconds

  @AC-461.4
  Scenario: Reconnection is logged with device info and timing
    Given a device that reconnects after a 10-second absence
    When the reconnection is processed
    Then the log SHALL contain the device stable ID, display name, and reconnection timestamp
