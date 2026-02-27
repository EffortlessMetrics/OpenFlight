@REQ-234 @product
Feature: Multiple clients can share axis inputs via cooperative profile  @AC-234.1
  Scenario: Cooperative profile defines left seat and right seat device roles
    Given a cooperative profile is loaded
    When the profile is parsed
    Then it SHALL declare distinct "left_seat" and "right_seat" device role assignments  @AC-234.2
  Scenario: Priority rules define which seat controls each axis
    Given a cooperative profile with priority rules for each axis
    When both seats provide input for the same axis simultaneously
    Then the axis value used by the RT spine SHALL be determined by the priority rules in the profile  @AC-234.3
  Scenario: Axis handoff between seats transitions smoothly
    Given control of an axis is being transferred from one seat to the other
    When the handoff occurs
    Then the axis output SHALL transition without an instantaneous snap or discontinuity  @AC-234.4
  Scenario: Both seats inputs visible in telemetry dashboard simultaneously
    Given two clients are connected in a cooperative session
    When telemetry is observed
    Then the dashboard SHALL display live input values from both the left seat and the right seat  @AC-234.5
  Scenario: IPC protocol supports two concurrent clients without conflict
    Given two IPC clients are connected to the service simultaneously
    When both clients issue commands concurrently
    Then the service SHALL process all commands without data corruption or connection errors  @AC-234.6
  Scenario: Disconnect of one client does not crash service
    Given two clients are connected in a cooperative session
    When one client disconnects unexpectedly
    Then the service SHALL continue running and the remaining client SHALL retain full control
