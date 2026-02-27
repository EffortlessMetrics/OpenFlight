@REQ-153 @product
Feature: Honeycomb Alpha yoke full integration  @AC-153.1
  Scenario: Alpha yoke XY axes report centre position
    Given the Honeycomb Alpha yoke is connected with VID 0x294B PID 0x1901
    When both roll and pitch axes are at mechanical centre
    Then the decoded X and Y axis values SHALL each be 0.0  @AC-153.2
  Scenario: Alpha yoke reports full right roll
    Given the Honeycomb Alpha yoke is connected
    When the roll axis is deflected to its rightmost stop
    Then the decoded X axis value SHALL be 1.0  @AC-153.3
  Scenario: Alpha yoke reports pitch up and pitch down extreme values
    Given the Honeycomb Alpha yoke is connected
    When the pitch axis is moved to its forward and aft stops in sequence
    Then the decoded Y axis SHALL reach -1.0 for pitch up and 1.0 for pitch down  @AC-153.4
  Scenario: All 23 Alpha buttons are decoded correctly
    Given the Honeycomb Alpha yoke is connected
    When each of the 23 buttons is pressed individually
    Then each button event SHALL carry the correct button index and pressed state  @AC-153.5
  Scenario: Gear switch position changes are decoded reliably
    Given the Honeycomb Alpha yoke is connected
    When the landing-gear switch is toggled between UP and DOWN
    Then each position change SHALL be reported as a distinct button event  @AC-153.6
  Scenario: Avionics master switch is decoded
    Given the Honeycomb Alpha yoke is connected
    When the avionics master switch is toggled
    Then the corresponding button event SHALL reflect the new switch state  @AC-153.7
  Scenario: Flap switch is decoded as a multi-position selector
    Given the Honeycomb Alpha yoke is connected
    When the flap switch is stepped through all positions
    Then each position SHALL produce a unique button index event  @AC-153.8
  Scenario: Device is recognised by VID and PID
    Given a HID enumeration is performed
    When a device with VID 0x294B and PID 0x1901 is found
    Then it SHALL be identified as the Honeycomb Alpha yoke
