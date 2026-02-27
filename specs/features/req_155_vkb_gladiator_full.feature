@REQ-155 @product
Feature: VKB Gladiator NXT EVO full integration  @AC-155.1
  Scenario: Stick XY axes report centre position with 14-bit resolution
    Given the VKB Gladiator NXT EVO is connected with VID 0x231D
    When both X and Y axes are at mechanical centre
    Then the decoded axis values SHALL each be 0.0 with 14-bit precision  @AC-155.2
  Scenario: Stick reports full right roll
    Given the VKB Gladiator NXT EVO is connected
    When the roll axis is deflected to its rightmost stop
    Then the decoded X axis value SHALL be 1.0  @AC-155.3
  Scenario: Twist Rz axis is decoded correctly
    Given the VKB Gladiator NXT EVO is connected
    When the twist axis is rotated through its full range
    Then the decoded Rz axis SHALL traverse the range -1.0 to 1.0  @AC-155.4
  Scenario: Hat switch all 8 positions are decoded
    Given the VKB Gladiator NXT EVO is connected
    When the hat switch is moved to each of the 8 positions
    Then each position SHALL produce a distinct decoded hat direction event  @AC-155.5
  Scenario: All 20 buttons are decoded correctly
    Given the VKB Gladiator NXT EVO is connected
    When each of the 20 buttons is pressed individually
    Then each button event SHALL carry the correct button index and pressed state  @AC-155.6
  Scenario: Throttle lever position is decoded
    Given the VKB Gladiator NXT EVO is connected
    When the throttle lever is moved across its full travel
    Then the decoded throttle axis SHALL report values across the full range  @AC-155.7
  Scenario: VKB Companion Hat is decoded
    Given the VKB Gladiator NXT EVO with Companion Hat is connected
    When the Companion Hat is moved to each available direction
    Then each direction SHALL produce a distinct decoded event  @AC-155.8
  Scenario: Device is recognised by VID
    Given a HID enumeration is performed
    When a device with VID 0x231D is found matching the Gladiator NXT EVO PID
    Then it SHALL be identified as a VKB Gladiator NXT EVO stick
