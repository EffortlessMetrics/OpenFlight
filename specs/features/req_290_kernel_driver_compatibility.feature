@REQ-290 @product
Feature: Kernel driver compatibility with hid-generic detection, udev rules, permission error guidance, and rootless operation  @AC-290.1
  Scenario: Service correctly identifies devices using hid-generic kernel driver
    Given a Linux host where a flight controller is bound to the hid-generic kernel driver
    When the service enumerates HID devices
    Then the device SHALL be detected and listed with its correct vendor ID and product ID  @AC-290.2
  Scenario: usbhid and hid-generic drivers coexist without conflicts
    Given a system where both usbhid and hid-generic bound devices are present simultaneously
    When the service initialises device access
    Then all devices SHALL be accessible and no driver conflict errors SHALL be reported  @AC-290.3
  Scenario: Devices requiring custom kernel modules are documented in compat matrix
    Given the compatibility matrix document for the project
    When it is inspected for entries referencing custom kernel module requirements
    Then all known devices that require a non-standard kernel module SHALL have an entry in the matrix  @AC-290.4
  Scenario: Service detects permission errors and provides actionable message
    Given a Linux host where the current user lacks read/write access to a HID device node
    When the service attempts to open that device
    Then it SHALL emit an error message describing the missing permission and instructing the user to add themselves to the flight group  @AC-290.5
  Scenario: udev rules grant read/write access to the flight group
    Given the installed udev rules for the service
    When a supported HID device is connected on Linux
    Then the device node SHALL have read/write permissions for the flight group as set by the udev rule  @AC-290.6
  Scenario: Service works without root privileges on Linux
    Given a Linux host with the udev rules installed and the current user in the flight group
    When the service is started as a non-root user
    Then it SHALL successfully open all supported HID devices and operate normally without requiring elevated privileges
