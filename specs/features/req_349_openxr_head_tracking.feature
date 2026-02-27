@REQ-349 @hardware @vr @head-tracking
Feature: OpenXR head tracking integration
  As a user with a VR headset
  I want head tracking to drive camera axes
  So that I can look around in-sim naturally

  Scenario: OpenXR runtime initialization  @AC-349.1
    Given an OpenXR-compatible runtime is installed
    When the system starts
    Then the OpenXR runtime is discovered and initialized
    And head pose data begins flowing within 2 seconds

  Scenario: Head pose read rate meets minimum  @AC-349.2
    Given the OpenXR session is active
    When head pose samples are collected over 1 second
    Then at least 90 samples SHALL have been received

  Scenario: Head pose published as HeadTrackingSnapshot  @AC-349.3
    Given the OpenXR session is active
    When a head pose frame is received
    Then a HeadTrackingSnapshot SHALL be published to the bus
    And the snapshot SHALL contain x, y, z, yaw, pitch, and roll fields

  Scenario: HMD disconnection triggers safe fallback  @AC-349.4
    Given the OpenXR session is active and tracking
    When the HMD connection is lost
    Then the system SHALL fall back to the last known pose
    And no panic or crash SHALL occur

  Scenario: Head tracking drives camera axis output  @AC-349.5
    Given a camera axis is mapped to the head tracking yaw channel
    When the user rotates their head 45 degrees to the right
    Then the camera axis output SHALL reflect the 45-degree rotation

  Scenario: OpenXR session lifecycle is managed  @AC-349.6
    Given the system is initializing
    When the OpenXR session is created and begun
    Then the session SHALL transition through create, begin, and running states
    And on shutdown the session SHALL be ended and destroyed cleanly
