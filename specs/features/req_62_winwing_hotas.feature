@REQ-62
Feature: WinWing HOTAS HID parsing

  @AC-62.1
  Scenario: WinWing device presets cover the full product range
    Given the WinWing preset tables for throttle, stick, and rudder categories
    When the preset counts are checked
    Then the throttle preset count SHALL match the number of supported throttle models
    And the stick preset count SHALL match the number of supported stick models
    And the rudder preset count SHALL match the number of supported rudder models
    And hall-effect sensor axes SHALL have a non-zero deadzone of no more than 0.05

  @AC-62.2
  Scenario: Orion2 Throttle and Stick reports parse correctly
    Given a minimal valid Orion2 Throttle report with both throttle levers at minimum (0)
    When parse_orion2_throttle_report is called
    Then throttle_left, throttle_right, and throttle_combined SHALL all be approximately 0.0
    And a report with both levers at maximum SHALL produce values approximately 1.0
    And throttle_combined SHALL equal the average of throttle_left and throttle_right
    And an Orion2 Stick report with centered raw axis values SHALL produce roll ≈ 0.0

  @AC-62.3
  Scenario: F-16EX Grip, SuperTaurus, Skywalker Rudder, and TFRP axes are within bounds
    Given a valid F-16EX Grip report with centered raw axis values
    When parse_f16ex_stick_report is called
    Then roll and pitch SHALL be approximately 0.0
    And full-right-roll raw value SHALL produce roll approximately 1.0
    And a SuperTaurus report with both throttle levers at 0 SHALL produce throttle_combined ≈ 0.0
    And Skywalker Rudder centered report SHALL produce all axes ≈ 0.0
    And differential brake SHALL be positive when right brake dominates and negative when left dominates
    And a WinWing TFRP centered report SHALL produce all axes ≈ 0.0

  @AC-62.4
  Scenario: Button detection in Orion2, F-16EX, and UFC Panel reports
    Given an F-16EX Grip report with button bit 0 set
    When parse_f16ex_stick_report is called
    Then is_pressed(1) SHALL return true and is_pressed(2) SHALL return false
    And an out-of-range button index SHALL return false
    And an Orion2 Stick report with button bit 0 set SHALL report button 1 as pressed
    And a UFC Panel report with UFC button 1 bit set SHALL report that button as pressed
    And a UFC Panel report with multiple button bits set SHALL report all of them simultaneously

  @AC-62.5
  Scenario: Short reports are rejected with a structured error
    Given an Orion2 Throttle HID buffer shorter than the minimum report length
    When parse_orion2_throttle_report is called
    Then a TooShort or similar error SHALL be returned and no panic SHALL occur
    And an empty buffer SHALL also return an error
    And parse_f16ex_stick_report, parse_super_taurus_report, parse_skywalker_rudder_report, parse_winwing_tfrp_report, and parse_ufc_panel_report SHALL all return errors for under-length buffers
