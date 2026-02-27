@REQ-146 @product
Feature: Elite Simulations hardware  @AC-146.1
  Scenario: Elite throttle quadrant axes parsed
    Given an Elite Simulations throttle quadrant connected via USB HID
    When a HID report with three lever positions is received
    Then the adapter SHALL parse all three throttle lever axes  @AC-146.2
  Scenario: Lever positions calibrated with known-good values
    Given an Elite throttle quadrant with known physical minimum and maximum positions
    When the calibration routine is run with these known-good values
    Then the adapter SHALL map lever positions to the full normalised range  @AC-146.3
  Scenario: Device connects as USB HID
    Given an Elite Simulations throttle quadrant plugged in to a USB port
    When the HID enumeration runs
    Then the device SHALL appear in the enumerated HID device list  @AC-146.4
  Scenario: Axis resolution appropriate for aviation use
    Given an Elite throttle quadrant producing axis data
    When the raw axis data is sampled
    Then each axis SHALL provide at least 10-bit resolution  @AC-146.5
  Scenario: Profile override applies for specific aircraft type
    Given an Elite throttle quadrant and a profile with a C172-specific override
    When a Cessna 172 is detected as the active aircraft
    Then the C172 profile override SHALL be applied to the throttle quadrant axes  @AC-146.6
  Scenario: Health check passes on all channels
    Given an Elite throttle quadrant connected and initialised
    When the device health check is invoked
    Then all axis channels SHALL report healthy status
