@REQ-114 @product
Feature: VIRPIL VPC Ace Interceptor Pedals device support

  @AC-114.1
  Scenario: Rudder axis at full left produces -1.0
    Given a VIRPIL VPC Ace Interceptor Pedals report with left pedal fully pressed
    When the rudder axis is computed from the pedal differential
    Then rudder SHALL be within 0.001 of -1.0

  @AC-114.1
  Scenario: Rudder axis at full right produces 1.0
    Given a VIRPIL VPC Ace Interceptor Pedals report with right pedal fully pressed
    When the rudder axis is computed from the pedal differential
    Then rudder SHALL be within 0.001 of 1.0

  @AC-114.2
  Scenario: Left toe brake axis is independent of right toe brake
    Given a VIRPIL VPC Ace Interceptor Pedals report with left toe brake at maximum and right toe brake at minimum
    When the report is parsed
    Then left_toe_brake SHALL be within 0.001 of 1.0
    And right_toe_brake SHALL be less than 0.001

  @AC-114.2
  Scenario: Right toe brake axis is independent of left toe brake
    Given a VIRPIL VPC Ace Interceptor Pedals report with right toe brake at maximum and left toe brake at minimum
    When the report is parsed
    Then right_toe_brake SHALL be within 0.001 of 1.0
    And left_toe_brake SHALL be less than 0.001

  @AC-114.3
  Scenario: 14-bit axis resolution yields at least 16384 distinct output values across full range
    Given a VIRPIL VPC Ace Interceptor Pedals device with 14-bit ADC resolution
    When the full raw range 0..=16383 is mapped to normalised output
    Then the number of distinct normalised values SHALL be at least 16383

  @AC-114.3
  Scenario: 14-bit axis values are always within [-1.0, 1.0] for any raw input
    Given any raw 14-bit pedal value in 0..=16383
    When the rudder axis is computed
    Then rudder SHALL be within [-1.0, 1.0]

  @AC-114.4
  Scenario: Center deadzone suppresses rudder output near mechanical center
    Given a VIRPIL VPC Ace Interceptor Pedals device with a 3% center deadzone configured
    When a raw pedal position within the deadzone band is parsed
    Then rudder SHALL be 0.0

  @AC-114.4
  Scenario: Calibration deadzone at center does not affect full-travel output
    Given a VIRPIL VPC Ace Interceptor Pedals device with a 3% center deadzone configured
    When the pedals are at maximum deflection
    Then rudder SHALL be within 0.001 of 1.0
