@REQ-61
Feature: Thrustmaster HOTAS HID parsing

  @AC-61.1
  Scenario: All Thrustmaster device models have valid presets
    Given the recommended_preset function for the Thrustmaster crate
    When called for every supported device model
    Then each preset SHALL have a deadzone within [0.0, 0.1]
    And each preset SHALL have a filter alpha within a reasonable range
    And the B104 preset SHALL carry the expected axis parameters

  @AC-61.2
  Scenario: Warthog stick and throttle decode axes and buttons correctly
    Given a Warthog stick report with centered axis raw values (0x8000)
    When parse_warthog_stick is called
    Then roll, pitch, and yaw axes SHALL be approximately 0.0
    And a full-right-deflection report SHALL produce roll ≈ 1.0
    And button bits set in the stick report SHALL be reflected in the button fields
    And a Warthog throttle idle report SHALL produce throttle axes ≈ 0.0
    And throttle button bits SHALL be decoded into the correct button fields

  @AC-61.3
  Scenario: Warthog axis outputs are always within normalized bounds
    Given proptest-generated Warthog stick reports with arbitrary u16 axis values
    When parse_warthog_stick and parse_warthog_throttle are called
    Then every axis value SHALL lie within [−1.0, 1.0]
    And normalize_axis_16bit of 0 SHALL return approximately −1.0
    And normalize_axis_8bit_centered of 128 SHALL return approximately 0.0
    And normalize_throttle of 0 SHALL return approximately 0.0

  @AC-61.4
  Scenario: T.16000M joystick and TWCS throttle parse correctly
    Given a T.16000M joystick report with centered axis values
    When the report is parsed
    Then roll and pitch axes SHALL be approximately 0.0
    And a full-right-deflection report SHALL produce roll ≈ 1.0
    And button bits SHALL be decoded into the correct boolean fields
    And a TWCS report with max throttle byte SHALL produce throttle ≈ 1.0
    And TWCS button bitmask SHALL be reflected in the output button fields

  @AC-61.5
  Scenario: Short reports are rejected with a structured error
    Given a Warthog stick HID buffer of only 9 bytes (minimum is 10)
    When parse_warthog_stick is called
    Then an error SHALL be returned and no panic SHALL occur
    And parse_warthog_throttle with a short buffer SHALL also return an error
    And the T.16000M and TWCS parsers SHALL return errors for under-length buffers
    And try_parse_report with an undersized buffer SHALL return Err
