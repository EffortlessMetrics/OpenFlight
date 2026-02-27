@REQ-490 @product
Feature: Axis Deadzone Visualization — ASCII Deadzone Display  @AC-490.1
  Scenario: flightctl axis show displays deadzone as ASCII range indicator
    Given an axis with a configured deadzone
    When `flightctl axis show` is executed
    Then the output SHALL include an ASCII range bar indicating the deadzone boundaries  @AC-490.2
  Scenario: Visualization shows current value position relative to deadzone
    Given an axis with a live value and a configured deadzone
    When `flightctl axis show` is executed
    Then the ASCII visualization SHALL indicate the current value position relative to the deadzone  @AC-490.3
  Scenario: Both symmetric and asymmetric deadzones are visualized correctly
    Given an axis configured with an asymmetric deadzone
    When `flightctl axis show` is executed
    Then the ASCII bar SHALL correctly reflect the asymmetric boundaries  @AC-490.4
  Scenario: Color coding distinguishes in-deadzone vs active range
    Given a terminal supporting color output
    When `flightctl axis show` is executed for an axis with a deadzone
    Then the in-deadzone region SHALL be rendered in a distinct color from the active range
