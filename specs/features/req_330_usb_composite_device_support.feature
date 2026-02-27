@REQ-330 @product
Feature: USB Composite Device Support  @AC-330.1
  Scenario: Service correctly handles composite HID devices with multiple interfaces
    Given a USB composite device presenting multiple HID interfaces
    When the device is connected
    Then the service SHALL enumerate all HID interfaces without error  @AC-330.2
  Scenario: Each interface is exposed as a separate logical device
    Given a composite device with two HID interfaces (e.g., joystick and throttle)
    When enumeration completes
    Then each interface SHALL appear as a distinct logical device in the device list  @AC-330.3
  Scenario: Interface selection is configurable in device config
    Given a composite device where only certain interfaces are relevant
    When the device config specifies interface indices to use
    Then the service SHALL expose only the configured interfaces as logical devices  @AC-330.4
  Scenario: Composite device disconnect removes all its logical devices
    Given a composite device with multiple active logical devices
    When the physical USB device is disconnected
    Then the service SHALL remove all logical devices associated with that composite device  @AC-330.5
  Scenario: VID/PID plus interface number is used as device identifier
    Given two interfaces of the same composite device (VID 044F, PID B10A)
    When the service assigns identifiers
    Then the identifiers SHALL be in the form VID_PID_IFn (e.g., 044F_B10A_IF0 and 044F_B10A_IF1)  @AC-330.6
  Scenario: Composite device is documented in compatibility matrix
    Given the compatibility matrix in the repository
    When a composite device is confirmed supported
    Then the compatibility matrix SHALL include an entry for that device noting composite interface support
