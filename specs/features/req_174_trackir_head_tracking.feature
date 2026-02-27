@REQ-174 @product
Feature: TrackIR head tracking integration

  @AC-174.1
  Scenario: TrackIR 5 device identified by VID and PID
    Given a HID device with the TrackIR 5 vendor ID and product ID
    When the device enumeration runs
    Then the device SHALL be identified as a TrackIR 5 head tracker

  @AC-174.2
  Scenario: 6DoF position data decoded from TrackIR frames
    Given a TrackIR 5 device is connected and tracking
    When a tracking frame is received
    Then all six degrees of freedom (X, Y, Z, yaw, pitch, roll) SHALL be decoded from the frame

  @AC-174.3
  Scenario: View axes updated at 120 Hz sample rate
    Given a TrackIR 5 device is connected and tracking
    When head tracking data is sampled
    Then the view axis values SHALL be updated at a rate of 120 Hz

  @AC-174.4
  Scenario: Deadzone applied at hardware neutral position
    Given a TrackIR 5 device with a deadzone configured at the hardware neutral position
    When the head is within the neutral deadzone region
    Then the view axis output SHALL be zero

  @AC-174.5
  Scenario: Lost tracking detected after N frames without update
    Given a TrackIR 5 device that stops sending valid tracking data
    When N consecutive frames pass without a valid tracking update
    Then the system SHALL detect and report the lost-tracking condition

  @AC-174.6
  Scenario: Profile remaps yaw to view offset axis
    Given a profile that remaps the TrackIR yaw channel to a view offset axis
    When the profile is loaded and the head is rotated in yaw
    Then the yaw data SHALL drive the configured view offset axis
