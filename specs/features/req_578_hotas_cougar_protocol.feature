@REQ-578 @product
Feature: HOTAS Cougar Protocol Support — Service should support Thrustmaster HOTAS Cougar protocol  @AC-578.1
  Scenario: HOTAS Cougar is identified by VID 0x044F and PID 0x0400
    Given a HID device is connected
    When the device has VID 0x044F and PID 0x0400
    Then the service SHALL identify it as a Thrustmaster HOTAS Cougar  @AC-578.2
  Scenario: Cougar 8-bit analog axes are correctly normalized
    Given a HOTAS Cougar device is active
    When the raw 8-bit axis value is read
    Then the service SHALL normalize it to the range -1.0 to 1.0  @AC-578.3
  Scenario: Cougar HAT switch maps to 8 discrete positions
    Given the HOTAS Cougar HAT switch is moved
    When the HID report is processed
    Then the service SHALL map the HAT input to one of 8 discrete directional positions  @AC-578.4
  Scenario: Cougar compatibility manifest includes known firmware quirks
    Given the Cougar device is loaded
    When the compatibility manifest is inspected
    Then it SHALL contain entries documenting known firmware quirks for the HOTAS Cougar
