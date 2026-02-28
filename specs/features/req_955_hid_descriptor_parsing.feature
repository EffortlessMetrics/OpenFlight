Feature: HID Descriptor Parsing
  As a flight simulation enthusiast
  I want hid descriptor parsing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Device capabilities are automatically detected from HID descriptors
    Given the system is configured for hid descriptor parsing
    When the feature is exercised
    Then device capabilities are automatically detected from HID descriptors

  Scenario: Parsed descriptors identify axis count, button count, and hat switches
    Given the system is configured for hid descriptor parsing
    When the feature is exercised
    Then parsed descriptors identify axis count, button count, and hat switches

  Scenario: Descriptor parsing handles vendor-specific usage pages gracefully
    Given the system is configured for hid descriptor parsing
    When the feature is exercised
    Then descriptor parsing handles vendor-specific usage pages gracefully

  Scenario: Parsed capabilities are cached to avoid repeated descriptor reads
    Given the system is configured for hid descriptor parsing
    When the feature is exercised
    Then parsed capabilities are cached to avoid repeated descriptor reads