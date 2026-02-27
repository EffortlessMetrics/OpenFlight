@REQ-111 @product
Feature: Heusinkveld Sprint Pedals device support

  @AC-111.1
  Scenario: Left brake pedal at full depression produces 100% output
    Given a Heusinkveld Sprint Pedals HID report with left brake raw at maximum
    When the report is parsed
    Then left_brake SHALL be within 0.001 of 1.0

  @AC-111.1
  Scenario: Right brake pedal at full depression produces 100% output
    Given a Heusinkveld Sprint Pedals HID report with right brake raw at maximum
    When the report is parsed
    Then right_brake SHALL be within 0.001 of 1.0

  @AC-111.1
  Scenario: Both brake pedals released produce 0% output
    Given a Heusinkveld Sprint Pedals HID report with both brake raws at minimum
    When the report is parsed
    Then left_brake SHALL be less than 0.001
    And right_brake SHALL be less than 0.001

  @AC-111.2
  Scenario: Toe brake differential is computed as left minus right
    Given a Heusinkveld Sprint Pedals report with left_brake raw at 75% and right_brake raw at 25%
    When the report is parsed
    Then toe_brake_differential SHALL be within 0.01 of 0.50

  @AC-111.3
  Scenario: Rudder axis is derived from left-right pedal differential
    Given a Heusinkveld Sprint Pedals report with left pedal fully pressed and right pedal fully released
    When the rudder axis is computed
    Then rudder SHALL be within 0.001 of -1.0

  @AC-111.3
  Scenario: Centered pedals produce zero rudder deflection
    Given a Heusinkveld Sprint Pedals report with both pedals at equal position
    When the rudder axis is computed
    Then rudder SHALL be within 0.01 of 0.0

  @AC-111.4
  Scenario: A calibration curve applied to the brake axes scales output correctly
    Given a Heusinkveld Sprint Pedals device with a linear calibration curve mapping [0, max] to [0.0, 0.9]
    When the left brake raw is at maximum
    Then the calibrated left_brake output SHALL be within 0.001 of 0.9

  @AC-111.4
  Scenario: Brake axis values are always within [0.0, 1.0] for any raw input
    Given any raw u16 brake value
    When the report is parsed with default calibration
    Then left_brake SHALL be within [0.0, 1.0]
    And right_brake SHALL be within [0.0, 1.0]

  @AC-111.5
  Scenario: Zero pedal input produces no rudder deflection
    Given a Heusinkveld Sprint Pedals report with both pedals at zero input
    When the rudder axis is computed
    Then rudder SHALL be within 0.001 of 0.0
