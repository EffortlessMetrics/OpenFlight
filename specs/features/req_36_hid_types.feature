@REQ-36
Feature: HID Device Info Types

  Background:
    Given the HID device info type system is available

  @AC-36.1
  Scenario: HidDeviceInfo construction with required and optional fields
    Given a HidDeviceInfo is constructed with required fields only
    Then the required fields should be set correctly
    And optional fields such as serial_number and manufacturer should default to None
    And the struct should be cloneable with all fields preserved

  @AC-36.1
  Scenario: HidDeviceInfo with descriptor data
    Given a HidDeviceInfo is constructed with a descriptor byte slice
    Then the descriptor field should contain the provided bytes
