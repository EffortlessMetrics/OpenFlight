@REQ-96 @product
Feature: Saitek Pro Flight Radio Panel COM/NAV Channel LED Management

  Background:
    Given the flight-panels-saitek crate with PanelType::RadioPanel, SaitekPanelWriter, and led_mapping

  @AC-96.1
  Scenario: Radio Panel product ID 0x0D05 is recognised as PanelType::RadioPanel
    Given PanelType::from_product_id is called with 0x0D05
    When the result is inspected
    Then the result SHALL be Some(PanelType::RadioPanel)
    And its human-readable name() SHALL equal "Radio Panel"

  @AC-96.2
  Scenario: Radio Panel LED mapping covers all COM, NAV, ADF, DME, and transponder channels
    Given PanelType::RadioPanel.led_mapping() is called
    When the returned slice is inspected
    Then it SHALL contain "COM1"
    And it SHALL contain "COM2"
    And it SHALL contain "NAV1"
    And it SHALL contain "NAV2"
    And it SHALL contain "ADF"
    And it SHALL contain "DME"
    And it SHALL contain "XPDR"

  @AC-96.2
  Scenario: Radio Panel can be registered in the SaitekPanelWriter and initialises LED states
    Given a SaitekPanelWriter and a device_info record for PanelType::RadioPanel
    When register_panel is called with that device_info
    Then the panel SHALL be present in the writer's panel registry
    And a LED-state entry SHALL exist for every name in RadioPanel.led_mapping()

  @AC-96.3
  Scenario: Radio Panel verify pattern cycles COM1 then NAV1 LEDs before clearing all
    Given PanelType::RadioPanel.verify_pattern() is called
    When the step sequence is inspected
    Then the first step SHALL be LedOn("COM1")
    And a subsequent step SHALL be LedOff("COM1")
    And a subsequent step SHALL be LedOn("NAV1")
    And the sequence SHALL end with AllOff

  @AC-96.3
  Scenario: Rate-limiting prevents more than one HID write per minimum interval
    Given a registered Radio Panel with min_write_interval of 8 ms
    When multiple LED state changes arrive within a single 8 ms window
    Then only one HID write SHALL be dispatched during that window
    And the next write SHALL be deferred until the interval elapses
