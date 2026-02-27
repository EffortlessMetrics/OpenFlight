@REQ-145 @product
Feature: 8BitDo controller support  @AC-145.1
  Scenario: 8BitDo Pro 2 thumbstick axes mapped
    Given an 8BitDo Pro 2 connected and producing HID reports
    When a report with left thumbstick X=0x80 and Y=0x40 is received
    Then the adapter SHALL map left thumbstick X and Y axis values correctly  @AC-145.2
  Scenario: Button mapping configurable via profile
    Given an 8BitDo Pro 2 with a loaded flight profile
    When the profile maps button A to "gear toggle"
    Then pressing button A SHALL trigger the gear toggle action  @AC-145.3
  Scenario: D-pad decoded as hat or buttons depending on profile
    Given an 8BitDo Pro 2 with a loaded flight profile
    When the profile specifies D-pad mode as "hat"
    Then D-pad inputs SHALL be decoded as a single hat-switch axis  @AC-145.4
  Scenario: Trigger axes normalised to 0 to 1 range
    Given an 8BitDo Pro 2 connected and producing HID reports
    When the right trigger reports its maximum raw value
    Then the adapter SHALL normalise the trigger axis to 1.0  @AC-145.5
  Scenario: Device identified by VID/PID
    Given a HID device enumeration result
    When a device with the 8BitDo Pro 2 VID/PID pair is present
    Then the adapter SHALL identify it as an 8BitDo Pro 2  @AC-145.6
  Scenario: Profile loads automatically on device connect
    Given a saved flight profile associated with the 8BitDo Pro 2
    When the device is connected
    Then the profile SHALL be loaded automatically
