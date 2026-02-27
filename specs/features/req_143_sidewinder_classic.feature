@REQ-143 @product
Feature: Microsoft Sidewinder classic controllers  @AC-143.1
  Scenario: Sidewinder FFB Pro detected by VID/PID
    Given a HID device enumeration result
    When a device with VID 0x045E PID 0x001B is present
    Then the adapter SHALL identify it as Microsoft Sidewinder FFB Pro  @AC-143.2
  Scenario: Sidewinder FFB2 detected by VID/PID
    Given a HID device enumeration result
    When a device with VID 0x045E PID 0x0038 is present
    Then the adapter SHALL identify it as Microsoft Sidewinder FFB2  @AC-143.3
  Scenario: Axes parsed from HID report
    Given a Sidewinder FFB Pro connected and producing HID reports
    When a HID report with pitch 0x200 and roll 0x300 is received
    Then the adapter SHALL parse pitch and roll axis values correctly  @AC-143.4
  Scenario: Force feedback constant effect applied
    Given a Sidewinder FFB Pro that supports DirectInput FFB
    When a constant force effect of magnitude 50% is requested
    Then the device SHALL report the effect as active  @AC-143.5
  Scenario: Device reconnect handled gracefully
    Given a Sidewinder FFB Pro that was previously connected
    When the device is disconnected and then reconnected
    Then the adapter SHALL re-initialise the device without returning an error  @AC-143.6
  Scenario: Legacy USB 1.0 polling at 10 ms is acceptable
    Given a Sidewinder FFB Pro operating over USB 1.0
    When the adapter polls the device at 10 ms intervals
    Then no latency violation SHALL be reported for USB 1.0 polling rates
