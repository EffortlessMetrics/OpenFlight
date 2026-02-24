@REQ-37
Feature: VKB Gladiator NXT EVO input parsing

  @AC-37.1
  Scenario: Parse Gladiator report with all axes populated
    Given a GladiatorInputHandler for the NxtEvoRight variant
    And a 21-byte HID report with known signed axis values and button bits
    When the report is parsed
    Then roll, pitch, yaw, throttle, mini_x, and mini_y SHALL be decoded to expected normalized values
    And the button bitmask SHALL match the expected pattern

  @AC-37.1
  Scenario: Parse Gladiator report with report ID prefix stripped
    Given a GladiatorInputHandler with report_id mode enabled
    And a 22-byte HID report with a Report ID prefix byte
    When the report is parsed
    Then the Report ID SHALL be stripped and axes SHALL decode correctly

  @AC-37.2
  Scenario: Bidirectional axes normalize symmetrically
    Given a GladiatorInputHandler for the NxtEvoLeft variant
    When raw axis value 0x0000 is parsed for roll
    Then roll SHALL normalize to approximately -1.0
    When raw axis value 0x8000 is parsed for roll
    Then roll SHALL normalize to approximately 0.0
    When raw axis value 0xFFFF is parsed for roll
    Then roll SHALL normalize to approximately +1.0

  @AC-37.2
  Scenario: Throttle wheel normalizes to unidirectional range
    Given a GladiatorInputHandler for the NxtEvoRight variant
    When raw throttle value 0x0000 is parsed
    Then throttle SHALL normalize to 0.0
    When raw throttle value 0xFFFF is parsed
    Then throttle SHALL normalize to 1.0

  @AC-37.3
  Scenario: POV hat decodes cardinal and diagonal directions
    Given a GladiatorInputHandler for the NxtEvoRight variant
    And a report with hat nibble value 0 (North)
    When the report is parsed
    Then hat 0 SHALL report direction 0 (North)
    And a report with hat nibble value 0xF (released) SHALL report None

  @AC-37.3
  Scenario: Report too-short error is returned for undersized payloads
    Given a GladiatorInputHandler expecting at least 12 bytes
    When a 5-byte report is presented
    Then a ReportTooShort error SHALL be returned with expected=12 and actual=5

  @AC-37.4
  Scenario: Buttons beyond first 32 decoded from second button word
    Given a GladiatorInputHandler for the NxtEvoRight variant
    And a 21-byte HID report with bit 32 set in the second button word
    When the report is parsed
    Then button index 33 SHALL be pressed
    And buttons 1-32 SHALL all be released

  @AC-37.5
  Scenario: pressed_buttons returns 1-based indices of active buttons
    Given a GladiatorInputState with buttons 1, 5, and 63 pressed
    When pressed_buttons is called
    Then the result SHALL contain exactly [1, 5, 63]
