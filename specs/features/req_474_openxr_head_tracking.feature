@REQ-474 @product
Feature: OpenXR Head Tracking Integration — 6DOF Head Pose via OpenXR  @AC-474.1
  Scenario: Service connects to OpenXR runtime when feature is enabled
    Given the openxr feature flag is enabled in config
    When the service starts and an OpenXR runtime is available
    Then the service SHALL create an OpenXR instance and session successfully  @AC-474.2
  Scenario: Head pose data is published as 6DOF values on flight-bus
    Given the OpenXR session is running
    When a new head pose frame is available
    Then the adapter SHALL publish yaw, pitch, roll, x, y, and z values as a BusSnapshot  @AC-474.3
  Scenario: Head tracking maps to configurable virtual axes
    Given a profile mapping OpenXR yaw to the virtual_pan_x axis
    When the user turns their head to the right
    Then the virtual_pan_x axis SHALL receive a proportional positive deflection  @AC-474.4
  Scenario: OpenXR session lifecycle follows runtime state machine
    Given an active OpenXR session
    When the runtime transitions to STOPPING state
    Then the adapter SHALL cleanly end the session and attempt to re-create it when the runtime becomes READY again
