@REQ-517 @product
Feature: Rudder Pedal Anti-Toe-Brake Mixing

  @AC-517.1 @AC-517.2
  Scenario: Anti-toe-brake mixing blends toe brake out of rudder axis
    Given anti-toe-brake mixing is enabled with a blending coefficient of 0.8
    When the pilot applies both rudder deflection and toe brake input
    Then the rudder axis output SHALL reduce proportionally to the toe brake input

  @AC-517.3
  Scenario: Differential braking mode preserves differential toe brake input
    Given anti-toe-brake mixing is enabled in differential braking mode
    When the pilot applies asymmetric left and right toe brake input
    Then the differential brake signal SHALL be preserved in the output

  @AC-517.4
  Scenario: Anti-toe-brake mixing is disabled by default
    Given a rudder pedal device is connected with default configuration
    When the axis pipeline processes rudder and toe brake inputs
    Then no mixing SHALL be applied between rudder and toe brake axes
