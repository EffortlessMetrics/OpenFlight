@REQ-495 @product
Feature: Gamepad Trigger Axis Support — Analog Trigger Axis Mapping  @AC-495.1
  Scenario: Left and right triggers are exposed as separate axes in 0.0 to 1.0 range
    Given a gamepad with analog triggers connected
    When the HID layer enumerates the device
    Then the left and right triggers SHALL each be exposed as independent axes in the 0.0–1.0 range  @AC-495.2
  Scenario: Triggers can be mapped to throttle or brake axes
    Given a gamepad trigger axis enumerated by the HID layer
    When a profile maps the trigger to a throttle or brake axis
    Then the processed axis value SHALL reflect the trigger position  @AC-495.3
  Scenario: Trigger dead zone is configurable
    Given a gamepad trigger axis with a configured dead zone
    When the trigger is within the dead zone range
    Then the processed axis output SHALL be zero  @AC-495.4
  Scenario: Dual trigger combo maps to split axis
    Given a profile configured to use dual-trigger split-axis mode
    When left and right triggers are moved simultaneously
    Then their combined values SHALL map to a single bipolar axis output
