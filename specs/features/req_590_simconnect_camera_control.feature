Feature: MSFS SimConnect Camera Control
  As a flight simulation enthusiast
  I want the SimConnect adapter to support camera control events
  So that I can map physical axes and buttons to camera movements in MSFS

  Background:
    Given the OpenFlight service is running
    And the SimConnect adapter is connected to MSFS

  Scenario: CAMERA_STATE SimConnect variable is readable
    When the SimConnect adapter polls for camera state
    Then the current value of the CAMERA_STATE variable is available

  Scenario: Camera look axis can be mapped to physical input axes
    Given a profile maps the "camera_look_x" axis to a physical joystick axis
    When the physical joystick axis moves to 0.5
    Then the SimConnect adapter sends a camera look X event with value 0.5

  Scenario: Camera mode change triggers a profile rule
    Given a profile rule triggers on camera mode change to "EXTERNAL"
    When the CAMERA_STATE variable changes to the external camera mode
    Then the matching profile rule fires

  Scenario: Camera control is enabled per-profile
    Given the active profile does not include a camera_control section
    When the SimConnect adapter processes camera events
    Then no camera control commands are sent to MSFS
