@REQ-95 @product
Feature: Saitek Pro Flight Switch Panel LED Control and HID Report Bit-Packing

  Background:
    Given the flight-panels-saitek crate with PanelType::SwitchPanel, SaitekPanelWriter, and led_mapping

  @AC-95.1
  Scenario: Switch Panel product ID 0x0D67 is recognised as PanelType::SwitchPanel
    Given PanelType::from_product_id is called with 0x0D67
    When the result is inspected
    Then the result SHALL be Some(PanelType::SwitchPanel)
    And its human-readable name() SHALL equal "Switch Panel"

  @AC-95.1
  Scenario: Unknown product IDs do not map to any panel type
    Given PanelType::from_product_id is called with an unknown PID (e.g. 0x9999)
    When the result is inspected
    Then the result SHALL be None

  @AC-95.2
  Scenario: Switch Panel LED mapping covers all eight safety-critical outputs
    Given PanelType::SwitchPanel.led_mapping() is called
    When the returned slice is inspected
    Then it SHALL contain "GEAR"
    And it SHALL contain "MASTER_BAT"
    And it SHALL contain "MASTER_ALT"
    And it SHALL contain "AVIONICS"
    And it SHALL contain "FUEL_PUMP"
    And it SHALL contain "DEICE"
    And it SHALL contain "PITOT_HEAT"
    And it SHALL contain "COWL"

  @AC-95.2
  Scenario: All five panel types have non-empty, uniquely-named LED mappings
    Given all five PanelType variants
    When led_mapping() is called for each
    Then every mapping SHALL be non-empty
    And no LED name SHALL appear twice within the same panel's mapping

  @AC-95.3
  Scenario: Switch Panel verify pattern starts with GEAR LED on then sequences to MASTER_BAT and AVIONICS
    Given PanelType::SwitchPanel.verify_pattern() is called
    When the step sequence is inspected
    Then the first step SHALL be LedOn("GEAR")
    And a subsequent step SHALL be LedOn("MASTER_BAT")
    And a subsequent step SHALL be LedOn("AVIONICS")
    And the final step SHALL be AllOff

  @AC-95.4
  Scenario: Switch Panel HID report encodes every LED index as a distinct set bit
    Given the SaitekPanelWriter and a registered Switch Panel device
    When each LED index from 0 to 7 is individually set to on
    Then each LED index SHALL set exactly one unique bit in the HID output report
    And no two indices SHALL share the same bit position

  @AC-95.4
  Scenario: All LEDs off produces a zero-bit HID report for the Switch Panel
    Given the SaitekPanelWriter and a registered Switch Panel device with all LEDs off
    When the HID output report is inspected
    Then the LED-mask byte SHALL be 0x00
