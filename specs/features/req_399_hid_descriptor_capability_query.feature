@REQ-399 @product
Feature: Hardware Capability Query via HID Descriptor — Read Axis Ranges from Descriptor

  @AC-399.1
  Scenario: HID descriptor is parsed to extract axis, button, and hat counts
    Given a connected HID device with a known descriptor
    When the descriptor is parsed
    Then axis count, button count, and hat count SHALL be correctly extracted

  @AC-399.2
  Scenario: Logical minimum and maximum are read per axis from the descriptor
    Given a HID descriptor containing axis usage pages
    When the descriptor is parsed
    Then the logical minimum and maximum SHALL be extracted for each axis

  @AC-399.3
  Scenario: Descriptor parse completes in under 5 ms
    Given a HID device descriptor of any length
    When the descriptor is parsed
    Then parsing SHALL complete in less than 5 ms

  @AC-399.4
  Scenario: Parse errors log the raw descriptor bytes and return a safe fallback
    Given a malformed or unrecognised HID descriptor
    When parsing fails
    Then the raw descriptor bytes SHALL be logged and a safe fallback SHALL be returned

  @AC-399.5
  Scenario: Descriptor-derived axis range is used for calibration baseline
    Given a HID device whose descriptor specifies axis logical ranges
    When the calibration baseline is computed
    Then it SHALL use the descriptor-derived logical minimum and maximum

  @AC-399.6
  Scenario: Property test — descriptor parser handles any byte sequence without panic
    Given the HID descriptor parser
    When any arbitrary byte sequence is passed as input
    Then the parser SHALL NOT panic
