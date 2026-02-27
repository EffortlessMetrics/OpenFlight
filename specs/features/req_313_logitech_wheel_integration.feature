@REQ-313 @product
Feature: Logitech Wheel Integration  @AC-313.1
  Scenario: Service recognizes G29/G920/G923 wheels via VID/PID
    Given a Logitech G29, G920, or G923 wheel is connected
    When the HID device enumeration runs
    Then the service SHALL recognize the wheel by its USB VID/PID and classify it as a supported wheel device  @AC-313.2
  Scenario: Wheel axis is exposed as yoke roll (steering to aileron)
    Given a Logitech wheel is recognized by the service
    When the wheel's steering axis input is received
    Then the service SHALL expose the steering axis as yoke roll mapped to aileron control  @AC-313.3
  Scenario: Pedal axes are exposed as throttle, brake, clutch
    Given a Logitech wheel with attached pedals is recognized
    When pedal axis inputs are received
    Then the service SHALL expose the pedal axes as throttle, brake, and clutch controls respectively  @AC-313.4
  Scenario: FFB effects are applied to wheel for flight sim feedback
    Given a Logitech wheel with force feedback capability is connected and a simulator is active
    When the simulator provides force feedback data
    Then the service SHALL apply the appropriate FFB effects to the wheel to represent flight sim feedback  @AC-313.5
  Scenario: Profile includes wheel-specific deadzone of 3-5% center
    Given a Logitech wheel profile is loaded
    When the profile's deadzone configuration is inspected
    Then the profile SHALL define a wheel-specific deadzone in the range of 3-5% at center position  @AC-313.6
  Scenario: Wheel is listed in compatibility matrix as Tier 2
    Given the compatibility matrix is loaded
    When the entry for Logitech G29, G920, or G923 is retrieved
    Then the wheel SHALL be listed with a Tier 2 support classification in the compatibility matrix
