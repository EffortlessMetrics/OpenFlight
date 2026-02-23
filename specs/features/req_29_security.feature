@REQ-29
Feature: Plugin security validation and authorization

  @AC-29.1
  Scenario: Plugin validation checks declared capabilities
    Given a plugin with declared capabilities
    When plugin validation is run
    Then plugins with permitted capabilities SHALL pass validation
    And plugins with forbidden capabilities SHALL fail validation

  @AC-29.2
  Scenario: Telemetry access is authorized by trust level
    Given a plugin with a specific trust level
    When telemetry access is requested
    Then high-trust plugins SHALL be authorized for sensitive telemetry
    And low-trust plugins SHALL be denied access to restricted telemetry

  @AC-29.3
  Scenario: Audit log records security events
    Given an active audit log
    When a security event occurs
    Then the event SHALL be appended to the audit log with a timestamp

  @AC-29.3
  Scenario: Audit log trims old entries
    Given an audit log that has grown beyond the retention limit
    When trimming is triggered
    Then only the most recent entries SHALL be retained
