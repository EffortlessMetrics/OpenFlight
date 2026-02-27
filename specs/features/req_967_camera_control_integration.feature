Feature: Camera Control Integration
  As a flight simulation enthusiast
  I want camera control integration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Head tracker input is forwarded to simulator camera control system
    Given the system is configured for camera control integration
    When the feature is exercised
    Then head tracker input is forwarded to simulator camera control system

  Scenario: Camera integration supports TrackIR, OpenTrack, and OpenXR protocols
    Given the system is configured for camera control integration
    When the feature is exercised
    Then camera integration supports TrackIR, OpenTrack, and OpenXR protocols

  Scenario: Camera smoothing parameters are configurable per simulator
    Given the system is configured for camera control integration
    When the feature is exercised
    Then camera smoothing parameters are configurable per simulator

  Scenario: Camera control does not interfere with normal axis processing pipeline
    Given the system is configured for camera control integration
    When the feature is exercised
    Then camera control does not interfere with normal axis processing pipeline