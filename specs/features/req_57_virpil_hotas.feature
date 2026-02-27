@REQ-57
Feature: VIRPIL VPC device input parsing

  @AC-57.1
  Scenario: CM3 Throttle all axes at zero parse to 0.0
    Given a VPC CM3 Throttle HID report with all axis raw values 0
    When parse_cm3_throttle_report is called
    Then left_throttle, right_throttle, flaps, scx, scy, and slider SHALL all be 0.0

  @AC-57.1
  Scenario: CM3 Throttle all axes at max parse to 1.0
    Given a VPC CM3 Throttle HID report with all axis raw values VIRPIL_AXIS_MAX
    When parse_cm3_throttle_report is called
    Then left_throttle, right_throttle, and flaps SHALL all be approximately 1.0

  @AC-57.1
  Scenario: CM3 Throttle half-position parses to approximately 0.5
    Given a VPC CM3 Throttle HID report with all axis raw values VIRPIL_AXIS_MAX/2
    When parse_cm3_throttle_report is called
    Then left_throttle SHALL be within 0.01 of 0.5

  @AC-57.2
  Scenario: CM3 Throttle button 1 detected
    Given a VPC CM3 Throttle HID report with bit 0 of button byte 0 set
    When parse_cm3_throttle_report is called
    Then is_pressed(1) SHALL return true
    And is_pressed(2) SHALL return false
    And pressed() SHALL return [1]

  @AC-57.2
  Scenario: CM3 Throttle button 78 detected
    Given a VPC CM3 Throttle HID report with bit 5 of button byte 9 set
    When parse_cm3_throttle_report is called
    Then is_pressed(78) SHALL return true

  @AC-57.2
  Scenario: CM3 Throttle all 78 buttons pressed
    Given a VPC CM3 Throttle HID report with all button bytes 0xFF
    When parse_cm3_throttle_report is called
    Then each button from 1 to 78 SHALL be reported as pressed

  @AC-57.2
  Scenario: CM3 Throttle out-of-range button indices return false
    Given a VPC CM3 Throttle HID report with all button bytes 0xFF
    When parse_cm3_throttle_report is called
    Then is_pressed(0) SHALL return false
    And is_pressed(79) SHALL return false

  @AC-57.2
  Scenario: CM3 Throttle no buttons pressed by default
    Given a VPC CM3 Throttle HID report with all button bytes 0x00
    When parse_cm3_throttle_report is called
    Then pressed() SHALL return an empty list

  @AC-57.3
  Scenario: MongoosT stick all axes at zero parse to 0.0
    Given a VPC MongoosT-50CM3 stick HID report with all axis raw values 0
    When parse_mongoost_stick_report is called
    Then x, y, and z SHALL all be 0.0

  @AC-57.3
  Scenario: MongoosT stick all axes at max parse to 1.0
    Given a VPC MongoosT-50CM3 stick HID report with all axis raw values VIRPIL_AXIS_MAX
    When parse_mongoost_stick_report is called
    Then x, y, z, sz, and sl SHALL all be approximately 1.0

  @AC-57.3
  Scenario: MongoosT stick hat decodes South direction
    Given a VPC MongoosT-50CM3 stick HID report with the high nibble of button byte 3 set to 4
    When parse_mongoost_stick_report is called
    Then hat SHALL be South

  @AC-57.3
  Scenario: MongoosT stick hat defaults to Center for unknown value
    Given a VPC MongoosT-50CM3 stick HID report with the high nibble of button byte 3 set to 0xF
    When parse_mongoost_stick_report is called
    Then hat SHALL be Center

  @AC-57.4
  Scenario: WarBRD Original variant identity preserved
    Given a VPC WarBRD HID report with variant Original
    When parse_warbrd_report is called
    Then state.variant SHALL be Original

  @AC-57.4
  Scenario: WarBRD-D variant identity preserved
    Given a VPC WarBRD HID report with variant D
    When parse_warbrd_report is called
    Then state.variant SHALL be D

  @AC-57.4
  Scenario: WarBRD product name for Original variant
    When product_name is called on WarBrdVariant::Original
    Then it SHALL return "VPC WarBRD Stick"

  @AC-57.4
  Scenario: WarBRD product name for D variant
    When product_name is called on WarBrdVariant::D
    Then it SHALL return "VPC WarBRD-D Stick"

  @AC-57.5
  Scenario: CM3 Throttle too-short report returns error
    Given a raw HID buffer of 22 bytes (below minimum of 23)
    When parse_cm3_throttle_report is called
    Then a TooShort error SHALL be returned

  @AC-57.5
  Scenario: MongoosT stick too-short report returns error
    Given a raw HID buffer of 14 bytes (below minimum of 15)
    When parse_mongoost_stick_report is called
    Then a TooShort error SHALL be returned

  @AC-57.5
  Scenario: WarBRD too-short report returns error
    Given a raw HID buffer of 14 bytes (below minimum of 15)
    When parse_warbrd_report is called with variant D
    Then a TooShort error SHALL be returned with length 14
