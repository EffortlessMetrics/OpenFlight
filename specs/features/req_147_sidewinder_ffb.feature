@REQ-147 @product
Feature: Windows Sidewinder FFB implementation  @AC-147.1
  Scenario: Sidewinder FFB Pro recognized by VID/PID
    Given a USB device with VID 0x045E and PID 0x001B attached
    When the HID enumeration runs
    Then the device SHALL be identified as a Sidewinder FFB Pro  @AC-147.2
  Scenario: Sidewinder FFB2 recognized by VID/PID
    Given a USB device with VID 0x045E and PID 0x001C attached
    When the HID enumeration runs
    Then the device SHALL be identified as a Sidewinder FFB2  @AC-147.3
  Scenario: Constant force effect applied at full magnitude
    Given a Sidewinder FFB device is initialised and ready
    When a constant force effect is commanded at full magnitude
    Then the FFB engine SHALL apply the constant force at 100% output  @AC-147.4
  Scenario: Spring centering effect dampens movements
    Given a Sidewinder FFB device with spring centering enabled
    When the joystick is displaced from centre
    Then the spring effect SHALL apply a restoring force proportional to displacement  @AC-147.5
  Scenario: Periodic sine effect oscillates at 10Hz
    Given a Sidewinder FFB device is initialised and ready
    When a periodic sine effect at 10 Hz is commanded
    Then the output SHALL oscillate sinusoidally at 10 Hz  @AC-147.6
  Scenario: FFB disabled gracefully on disconnect
    Given a Sidewinder FFB device with an active force effect
    When the device is disconnected
    Then all force output SHALL be stopped and the FFB session SHALL be released cleanly
