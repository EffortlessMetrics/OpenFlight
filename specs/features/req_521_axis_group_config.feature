@REQ-521 @product
Feature: Axis Group Configuration

  @AC-521.1 @AC-521.2
  Scenario: Axes in a group share synchronized deadzone and curve application
    Given two throttle axes assigned to the group "dual-throttle"
    When a deadzone of 3% is configured for the group
    Then both axes SHALL apply the same 3% deadzone synchronously each tick

  @AC-521.3
  Scenario: Group master-slave mode allows one axis to lead scaling
    Given a group "dual-throttle" with axis A set as master
    When axis A is scaled by a factor of 0.9
    Then axis B SHALL apply the same scaling factor derived from axis A

  @AC-521.4
  Scenario: Group configuration is stored in profile
    Given the user configures an axis group in the UI
    When the profile is saved
    Then the profile file SHALL contain the group name and member axis assignments
