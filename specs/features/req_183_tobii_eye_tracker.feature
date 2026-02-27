@REQ-183 @product
Feature: Tobii Eye Tracker provides gaze-based view offset in flight sim  @AC-183.1
  Scenario: Tobii Eye Tracker 5 detected as gaze input device
    Given a Tobii Eye Tracker 5 is connected
    When the HID subsystem enumerates input devices
    Then the Eye Tracker 5 SHALL be detected and registered as a gaze input device  @AC-183.2
  Scenario: Gaze vector normalized to view offset range
    Given a Tobii Eye Tracker 5 is active and tracking the user's gaze
    When the user looks to any point within the trackable area
    Then the gaze vector SHALL be normalized to the range [-1.0, 1.0] as a view offset per axis  @AC-183.3
  Scenario: Gaze tracking operates within RT spine budget
    Given a Tobii Eye Tracker 5 is delivering gaze data
    When gaze updates arrive at their native rate
    Then gaze tracking SHALL operate at 30-90Hz without overloading the RT spine  @AC-183.4
  Scenario: 6DOF head position available as separate axis set
    Given a Tobii Eye Tracker 5 that supports head tracking
    When head position data is being streamed
    Then the 6DOF head position SHALL be available as a separate axis set distinct from gaze  @AC-183.5
  Scenario: Tracking loss returns view to center
    Given a Tobii Eye Tracker 5 is active and providing gaze data
    When gaze tracking is lost due to the user looking away or occlusion
    Then the view offset SHALL return to center gracefully without abrupt jumps  @AC-183.6
  Scenario: Tobii SDK isolated behind port trait
    Given the OpenFlight Tobii integration is compiled
    When the Tobii SDK is inspected for coupling
    Then the SDK integration SHALL be isolated behind an OpenFlight port trait with no direct SDK types leaking into core
