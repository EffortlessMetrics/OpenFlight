@REQ-572 @product
Feature: Garmin G1000 Panel Mapping — Service should support Garmin G1000 replica panel button mapping  @AC-572.1
  Scenario: G1000 panel buttons map to sim events via profile rules
    Given a G1000 replica panel is connected
    When a panel button is pressed
    Then the corresponding sim event SHALL be triggered according to the active profile rules  @AC-572.2
  Scenario: Encoder rotation maps to increment/decrement events
    Given a G1000 encoder is rotated clockwise or counter-clockwise
    When the HID report is processed
    Then the service SHALL emit an increment or decrement event respectively  @AC-572.3
  Scenario: Long-press detection maps to alternate function
    Given a G1000 panel button is held for the long-press threshold duration
    When the long-press is detected
    Then the service SHALL map the input to the alternate function defined in the profile  @AC-572.4
  Scenario: G1000 panel is discoverable via HID enumeration
    Given the HID subsystem is running
    When devices are enumerated
    Then the G1000 replica panel SHALL appear in the device list
