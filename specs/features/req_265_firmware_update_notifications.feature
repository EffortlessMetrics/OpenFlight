@REQ-265 @product
Feature: Firmware update notifications alert operator without blocking device operation  @AC-265.1

  Scenario: Service detects firmware version mismatch on device connect
    Given a device with firmware version 1.0.0 connects and the manifest specifies target version 1.2.0
    When the service processes the device connection
    Then the service SHALL detect the firmware mismatch and record it in the device state

  Scenario: Firmware mismatch notification emitted on gRPC health stream
    Given a device with a firmware version mismatch has connected
    When a client subscribes to the gRPC health event stream
    Then the stream SHALL emit a FirmwareUpdateAvailable event with device identifier  @AC-265.2

  Scenario: CLI displays firmware update available message
    Given a device with a firmware mismatch is connected and the service is running
    When the operator runs `flightctl devices`
    Then the CLI output SHALL include a firmware update available notice for the device  @AC-265.3

  Scenario: Notification includes device name and version details
    Given a firmware mismatch notification is generated for device "Thrustmaster HOTAS Warthog"
    When the notification payload is inspected
    Then it SHALL include the device name, current firmware version, and target firmware version  @AC-265.4

  Scenario: Operator suppresses notifications for a specific device
    Given the operator has configured suppression for VID 0x044F PID 0x0402
    When that device connects with a firmware mismatch
    Then no firmware notification SHALL be emitted for that device  @AC-265.5

  Scenario: Firmware check does not block device input
    Given a device connects and a firmware version check is initiated
    When axis input is sampled during the firmware check
    Then axis data SHALL be available on the bus before the firmware check completes  @AC-265.6
