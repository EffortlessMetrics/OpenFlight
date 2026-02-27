@REQ-209 @product
Feature: WinWing devices fully integrated with correct axes, buttons, and MFD protocol  @AC-209.1
  Scenario: WinWing FCU panel axes detected and normalized
    Given a WinWing FCU panel connected via USB
    When the device enumerates
    Then all FCU panel axes SHALL be detected and normalized to the standard range  @AC-209.2
  Scenario: WinWing TECS throttle dual throttle axes processed independently
    Given a WinWing TECS throttle with two physical throttle levers
    When both throttle levers are moved independently
    Then each axis SHALL be processed and reported independently without coupling  @AC-209.3
  Scenario: WinWing UFC panel soft-key matrix events routed to rules engine
    Given a WinWing UFC panel with a soft-key matrix
    When a soft-key is pressed
    Then the event SHALL be routed to the rules engine for processing  @AC-209.4
  Scenario: WinWing Ursa Minor F-16 throttle parseable with correct PIDs
    Given a WinWing Ursa Minor F-16 throttle connected
    When the HID descriptor is parsed using the known PIDs
    Then all axes and buttons SHALL be correctly identified and mapped  @AC-209.5
  Scenario: WinWing panel LEDs controllable via profile LED state machine
    Given a WinWing panel with controllable LEDs
    When the profile LED state machine transitions to a new state
    Then the panel LEDs SHALL reflect the new state as configured in the profile  @AC-209.6
  Scenario: WinWing USB enumeration resilient to hub resets
    Given a WinWing device connected via a USB hub
    When the USB hub is reset
    Then the device SHALL re-enumerate automatically and resume normal operation
