@REQ-688
Feature: Axis Range Soft Limits
  @AC-688.1
  Scenario: Soft limits constrain output to a sub-range of full travel
    Given the system is configured for REQ-688
    When the feature condition is met
    Then soft limits constrain output to a sub-range of full travel

  @AC-688.2
  Scenario: Soft limits are defined in profile YAML as min_out and max_out
    Given the system is configured for REQ-688
    When the feature condition is met
    Then soft limits are defined in profile yaml as min_out and max_out

  @AC-688.3
  Scenario: Values outside soft limits are clamped not rejected
    Given the system is configured for REQ-688
    When the feature condition is met
    Then values outside soft limits are clamped not rejected

  @AC-688.4
  Scenario: Soft limit application is the second-to-last pipeline stage
    Given the system is configured for REQ-688
    When the feature condition is met
    Then soft limit application is the second-to-last pipeline stage
