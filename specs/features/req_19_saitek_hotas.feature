@REQ-19
Feature: Saitek HOTAS input parsing and health monitoring

  @AC-19.1
  Scenario: Normalize 8-bit axis values to -1.0..1.0
    Given raw 8-bit axis values 0, 127, and 255
    When each value is normalized
    Then 0 SHALL map to approximately -1.0
    And 127 SHALL map to approximately 0.0
    And 255 SHALL map to approximately 1.0

  @AC-19.1
  Scenario: Normalize 16-bit axis values to -1.0..1.0
    Given raw 16-bit axis values 0, 32767, and 65535
    When each value is normalized
    Then 0 SHALL map to approximately -1.0
    And 32767 SHALL map to approximately 0.0
    And 65535 SHALL map to approximately 1.0

  @AC-19.2
  Scenario: Create X52 Pro input handler
    Given a HOTAS device type of X52Pro
    When a HotasInputHandler is created
    Then the device type SHALL be X52Pro
    And the ghost input rate SHALL be 0.0 at creation time

  @AC-19.3
  Scenario: Health monitor records success and resets counter
    Given a Saitek HOTAS health monitor
    When one failure is recorded followed by a success
    Then the consecutive failure count SHALL reset to zero

  @AC-19.3
  Scenario: Health monitor detects failure threshold exceeded
    Given a Saitek HOTAS health monitor with default threshold of 3
    When 3 consecutive failures are recorded
    Then the failure threshold SHALL be reported as exceeded

  @AC-19.3
  Scenario: Health status reflects healthy device state
    Given a Saitek HOTAS health monitor with no failures
    When a health status is constructed with connected=true and low ghost rate
    Then is_healthy SHALL return true

  @AC-19.3
  Scenario: Health status detects ghost input issues
    Given a Saitek HOTAS health status with a ghost rate above 5%
    When has_ghost_issues is checked
    Then it SHALL return true
