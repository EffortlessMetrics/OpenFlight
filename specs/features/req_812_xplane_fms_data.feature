Feature: X-Plane FMS Data
  As a flight simulation enthusiast
  I want x-plane fms data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Read active FMS route waypoints
    Given the system is configured for x-plane fms data
    When the feature is exercised
    Then x-Plane adapter reads active FMS route waypoints and sequences

  Scenario: Expose GPS deviation and distance-to-next
    Given the system is configured for x-plane fms data
    When the feature is exercised
    Then gPS course deviation and distance-to-next are exposed as variables

  Scenario: Publish FMS updates on change
    Given the system is configured for x-plane fms data
    When the feature is exercised
    Then fMS data updates are published to the event bus on change

  Scenario: Handle route changes mid-flight
    Given the system is configured for x-plane fms data
    When the feature is exercised
    Then adapter handles FMS route changes mid-flight without errors
