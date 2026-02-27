@REQ-97 @product
Feature: Saitek Pro Flight Instrument Panel (FIP) Instrument Channel Identification

  Background:
    Given the flight-panels-saitek crate with PanelType::FIP and led_mapping

  @AC-97.1
  Scenario: FIP product ID 0x0A2F is recognised as PanelType::FIP
    Given PanelType::from_product_id is called with 0x0A2F
    When the result is inspected
    Then the result SHALL be Some(PanelType::FIP)
    And its human-readable name() SHALL equal "Flight Instrument Panel"

  @AC-97.2
  Scenario: FIP LED mapping covers all primary flight and navigation instrument channels
    Given PanelType::FIP.led_mapping() is called
    When the returned slice is inspected
    Then it SHALL contain "ATTITUDE"
    And it SHALL contain "AIRSPEED"
    And it SHALL contain "ALTITUDE"
    And it SHALL contain "HSI"
    And it SHALL contain "TURN_COORD"
    And it SHALL contain "VOR1"
    And it SHALL contain "VOR2"
    And it SHALL contain "ADF"

  @AC-97.3
  Scenario: FIP verify pattern activates ATTITUDE, AIRSPEED, ALTITUDE together then clears all
    Given PanelType::FIP.verify_pattern() is called
    When the step sequence is inspected
    Then the first step SHALL be LedOn("ATTITUDE")
    And the second step SHALL be LedOn("AIRSPEED")
    And the third step SHALL be LedOn("ALTITUDE")
    And a subsequent step SHALL be a Delay
    And the final step SHALL be AllOff

  @AC-97.3
  Scenario: FIP can be registered alongside other panel types without LED-state collisions
    Given a SaitekPanelWriter with a registered FIP and a registered RadioPanel
    When LED states are set independently for each panel
    Then LED state changes on the FIP SHALL NOT affect LED states on the RadioPanel
    And LED state changes on the RadioPanel SHALL NOT affect LED states on the FIP
