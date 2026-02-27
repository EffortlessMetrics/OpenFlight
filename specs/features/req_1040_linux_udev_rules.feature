@REQ-1040
Feature: Linux Udev Rules
  @AC-1040.1
  Scenario: Udev rules are provided for automatic device permission configuration
    Given the system is configured for REQ-1040
    When the feature condition is met
    Then udev rules are provided for automatic device permission configuration

  @AC-1040.2
  Scenario: Rules grant appropriate access to HID and USB devices
    Given the system is configured for REQ-1040
    When the feature condition is met
    Then rules grant appropriate access to hid and usb devices

  @AC-1040.3
  Scenario: Rule installation is automated during package installation
    Given the system is configured for REQ-1040
    When the feature condition is met
    Then rule installation is automated during package installation

  @AC-1040.4
  Scenario: Rules can be regenerated via CLI for newly supported devices
    Given the system is configured for REQ-1040
    When the feature condition is met
    Then rules can be regenerated via cli for newly supported devices
