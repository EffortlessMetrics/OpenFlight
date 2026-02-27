@REQ-177 @product
Feature: TrackIR head tracking updates head position in real time

  @AC-177.1
  Scenario: TrackIR position packets received on NaturalPoint protocol
    Given a TrackIR device is connected and the NaturalPoint client library is active
    When the head tracker transmits a position packet
    Then the system SHALL receive and parse the packet on the NaturalPoint protocol

  @AC-177.2
  Scenario: Yaw, pitch and roll normalized to [-1.0, 1.0]
    Given a TrackIR device is supplying head orientation data
    When yaw, pitch, and roll values span their full hardware range
    Then each axis value SHALL be normalized to the range [-1.0, 1.0]

  @AC-177.3
  Scenario: Head tracking paused and resumed without service restart
    Given the flight service is running with TrackIR head tracking active
    When a pause command is issued followed by a resume command
    Then head tracking SHALL stop delivering updates while paused and resume delivering updates after resume without restarting the service

  @AC-177.4
  Scenario: Stale TrackIR data detected after 500 ms timeout
    Given a TrackIR device is supplying head tracking data
    When no new position packet is received for 500 ms
    Then the system SHALL mark the TrackIR data as stale and surface a timeout event

  @AC-177.5
  Scenario: TrackIR mapped to dedicated virtual axes in the axis engine
    Given head tracking is enabled in the profile with virtual axis assignments for yaw, pitch, and roll
    When TrackIR position data is received
    Then the axis engine SHALL expose the values on the dedicated virtual head-tracking axes

  @AC-177.6
  Scenario: Head tracking active simultaneously with flight stick inputs
    Given a flight stick and a TrackIR device are both connected
    When the flight stick axes are moved while head tracking is active
    Then both input streams SHALL be processed independently without interfering with each other
