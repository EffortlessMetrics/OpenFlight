@REQ-295 @product
Feature: Haptic Feedback Integration  @AC-295.1
  Scenario: Service supports tactile feedback devices
    Given a D-BOX or ButtKicker device is connected and configured
    When the service starts
    Then the haptic device SHALL be enumerated and shown as active in the device list  @AC-295.2
  Scenario: Haptic intensity is proportional to simulated g-force or event severity
    Given a profile with haptic intensity linked to simulator g-force telemetry
    When the simulator reports a 3G manoeuvre
    Then the haptic device output intensity SHALL scale proportionally to the reported g-force  @AC-295.3
  Scenario: Haptic cues triggered by zone crossings
    Given a profile with a haptic cue configured for the "landing_gear_down" zone crossing event
    When the simulator signals the landing gear extending
    Then the haptic device SHALL produce the configured cue within 50ms  @AC-295.4
  Scenario: Haptic can be configured independently of FFB
    Given a profile with FFB spring centering enabled and haptic friction pattern set to "off"
    When both configurations are active simultaneously
    Then FFB and haptic SHALL operate independently without mutual interference  @AC-295.5
  Scenario: Haptic devices are enumerated separately from flight controls
    Given a ButtKicker and a joystick are both connected
    When the ListDevices gRPC RPC is called
    Then the ButtKicker SHALL appear under device type "haptic" and the joystick under its own type  @AC-295.6
  Scenario: Haptic is disabled when simulator disconnects
    Given the haptic device is active and producing output linked to simulator events
    When the simulator connection drops
    Then the haptic device output SHALL stop within one second of the disconnect
