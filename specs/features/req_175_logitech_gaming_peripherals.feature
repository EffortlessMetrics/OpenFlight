@REQ-175 @product
Feature: Logitech G502 and G Pro gaming peripherals as flight inputs

  @AC-175.1
  Scenario: G502 extra buttons decoded as HID inputs
    Given a Logitech G502 gaming mouse is connected
    When any of the extra programmable buttons are pressed
    Then each button SHALL be decoded as an individual HID input event

  @AC-175.2
  Scenario: G502 scroll wheel mapped to axis increment
    Given a Logitech G502 gaming mouse with the scroll wheel mapped to an axis increment in the profile
    When the scroll wheel is rotated
    Then the configured axis value SHALL increment or decrement accordingly

  @AC-175.3
  Scenario: Pro Racing Wheel axes normalized correctly
    Given a Logitech G Pro Racing Wheel is connected
    When the steering and pedal axes are moved across their full ranges
    Then all axis values SHALL be normalized to their correct output ranges

  @AC-175.4
  Scenario: Gaming mouse used as trim knob input
    Given a gaming mouse scroll wheel is mapped to a trim axis in the profile
    When the scroll wheel is rotated
    Then the trim axis SHALL change by the configured increment per scroll detent

  @AC-175.5
  Scenario: Device connects as standard HID without custom driver
    Given a supported Logitech gaming peripheral is connected
    When the device is enumerated
    Then the device SHALL be accessible via the standard HID interface without requiring a custom driver

  @AC-175.6
  Scenario: Profile assigns gaming buttons to sim commands
    Given a profile that assigns G502 extra buttons to simulator commands
    When a mapped button is pressed
    Then the corresponding simulator command SHALL be triggered
