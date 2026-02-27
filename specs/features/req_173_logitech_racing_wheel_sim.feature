@REQ-173 @product
Feature: Logitech G25/G27 racing wheel in flight sim

  @AC-173.1
  Scenario: G25 16-bit wheel axis normalized to [-1, 1]
    Given a Logitech G25 racing wheel is connected
    When the steering wheel is moved across its full range
    Then the 16-bit wheel axis value SHALL be normalized to the range [-1, 1]

  @AC-173.2
  Scenario: G27 16-bit wheel axis normalized to [-1, 1]
    Given a Logitech G27 racing wheel is connected
    When the steering wheel is moved across its full range
    Then the 16-bit wheel axis value SHALL be normalized to the range [-1, 1]

  @AC-173.3
  Scenario: G25 pedals normalized to [0, 1]
    Given a Logitech G25 racing wheel with pedals connected
    When a pedal is fully depressed
    Then the pedal axis value SHALL be normalized to the range [0, 1]

  @AC-173.4
  Scenario: G27 provides three independent pedal axes
    Given a Logitech G27 racing wheel with pedals connected
    When the accelerator, brake, and clutch pedals are each depressed independently
    Then three separate normalized axis values SHALL be available for accelerator, brake, and clutch

  @AC-173.5
  Scenario: Profile remaps wheel axis to bank and pitch controls
    Given a profile that remaps the G25/G27 wheel axis to bank and pitch flight axes
    When the profile is loaded and the wheel is moved
    Then the wheel axis SHALL drive the configured bank and pitch axes

  @AC-173.6
  Scenario: Device identified by Logitech VID 0x046D
    Given a HID device with vendor ID 0x046D matching a G25 or G27 product ID
    When the device enumeration runs
    Then the device SHALL be identified as a Logitech racing wheel
