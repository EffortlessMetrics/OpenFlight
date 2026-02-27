@REQ-172 @product
Feature: Nintendo Switch controller HID support

  @AC-172.1
  Scenario: Switch Pro left stick X and Y axes mapped to flight axes
    Given a Nintendo Switch Pro Controller is connected
    When the left stick is moved along the X and Y axes
    Then both axes SHALL be decoded and mapped to the configured flight control axes

  @AC-172.2
  Scenario: Switch Pro ZL and ZR triggers normalized to [0, 1]
    Given a Nintendo Switch Pro Controller is connected
    When the ZL or ZR trigger is fully depressed
    Then the trigger axis value SHALL be normalized to the range [0, 1]

  @AC-172.3
  Scenario: Joy-Con L horizontal axis decoded
    Given a Nintendo Joy-Con (L) is connected
    When the horizontal stick axis is moved
    Then the horizontal axis value SHALL be decoded and available as a normalized flight input

  @AC-172.4
  Scenario: Joy-Con R vertical axis decoded
    Given a Nintendo Joy-Con (R) is connected
    When the vertical stick axis is moved
    Then the vertical axis value SHALL be decoded and available as a normalized flight input

  @AC-172.5
  Scenario: Gyro data optionally available as axis inputs
    Given a Nintendo Switch Pro Controller with gyroscope enabled in the profile
    When the controller is rotated
    Then gyro rate data SHALL be available as optional axis inputs

  @AC-172.6
  Scenario: Device identified by VID 0x057E PID 0x2009
    Given a HID device with vendor ID 0x057E and product ID 0x2009
    When the device enumeration runs
    Then the device SHALL be identified as a Nintendo Switch Pro Controller
