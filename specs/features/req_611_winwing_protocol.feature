Feature: WinWing Device Protocol Support
  As a flight simulation enthusiast
  I want WinWing devices to be fully supported
  So that I can use WinWing hardware with OpenFlight

  Background:
    Given the OpenFlight service is running

  Scenario: WinWing devices are identified by VID 0x4098
    When a device with VID 0x4098 is connected
    Then the service identifies it as a WinWing device

  Scenario: WinWing LED state can be set via HID output report
    Given a WinWing device is connected
    When an LED state change is requested
    Then the service sends the appropriate HID output report to the device

  Scenario: WinWing button binding config is documented
    When the user queries documentation for WinWing button bindings
    Then the documentation describes the available binding configuration options

  Scenario: WinWing compatibility manifests cover all major products
    When the compatibility manifest list is inspected
    Then manifests exist for all major WinWing product lines
