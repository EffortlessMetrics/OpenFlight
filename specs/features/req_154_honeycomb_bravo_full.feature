@REQ-154 @product
Feature: Honeycomb Bravo throttle full integration  @AC-154.1
  Scenario: Bravo six throttle axes report idle position
    Given the Honeycomb Bravo throttle is connected with VID 0x294B PID 0x1905
    When all six throttle levers are positioned at idle
    Then each decoded throttle axis value SHALL be 0.0  @AC-154.2
  Scenario: Bravo six throttle axes report full-forward position
    Given the Honeycomb Bravo throttle is connected
    When all six throttle levers are advanced to full forward
    Then each decoded throttle axis value SHALL be 1.0  @AC-154.3
  Scenario: Mixed throttle positions are decoded independently
    Given the Honeycomb Bravo throttle is connected
    When throttle levers are set to different positions simultaneously
    Then each axis SHALL report its own independent decoded value  @AC-154.4
  Scenario: Prop levers are decoded as separate axes
    Given the Honeycomb Bravo throttle is connected
    When each prop lever is moved across its full travel
    Then each prop lever SHALL map to a distinct decoded axis  @AC-154.5
  Scenario: Mixture levers are decoded as separate axes
    Given the Honeycomb Bravo throttle is connected
    When each mixture lever is moved across its full travel
    Then each mixture lever SHALL map to a distinct decoded axis  @AC-154.6
  Scenario: Gear lever position is decoded
    Given the Honeycomb Bravo throttle is connected
    When the gear lever is moved between its positions
    Then each position SHALL be reported as a discrete axis or button event  @AC-154.7
  Scenario: Annunciator LED panel illumination is controlled
    Given the Honeycomb Bravo throttle is connected
    When an LED illumination command is sent for a specific annunciator
    Then the corresponding LED SHALL be activated via the HID output report  @AC-154.8
  Scenario: Device is identified by VID and PID
    Given a HID enumeration is performed
    When a device with VID 0x294B and PID 0x1905 is found
    Then it SHALL be identified as the Honeycomb Bravo throttle
